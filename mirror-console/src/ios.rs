//! iOS control via the open-source WebDriverAgent (WDA).
//!
//! This is original client code. It does NOT reuse any third-party app's tap
//! agent — it speaks the standard WebDriver/WDA HTTP API to a WDA instance the
//! user runs on their Mac (built from https://github.com/appium/WebDriverAgent).
//! Reach the device by forwarding its WDA port to localhost, e.g.:
//!
//!   go-ios forward 8100 8100 --udid <UDID>      # or: iproxy 8100 8100
//!
//! Then WDA is at 127.0.0.1:8100. Override with MC_WDA_HOST / MC_WDA_PORT.
//!
//! Coordinates for tap/swipe are normalized 0..1 in screen space (like a
//! generic remote pointer); they are converted to device points using WDA's
//! reported window size.

use std::env;
use std::fs;
use std::io::{Read, Write};
use std::net::TcpStream;
use std::path::PathBuf;
use std::time::Duration;

fn host() -> String {
    env::var("MC_WDA_HOST").unwrap_or_else(|_| "127.0.0.1".to_string())
}
fn port() -> u16 {
    env::var("MC_WDA_PORT").ok().and_then(|p| p.parse().ok()).unwrap_or(8100)
}

// ---------- minimal HTTP/1.1 client (localhost JSON) ----------

fn http(method: &str, path: &str, body: Option<&str>) -> Result<(u16, String), String> {
    let addr = format!("{}:{}", host(), port());
    let mut stream = TcpStream::connect(&addr)
        .map_err(|e| format!("cannot reach WDA at {addr}: {e} (is the port forwarded?)"))?;
    stream.set_read_timeout(Some(Duration::from_secs(60))).ok();
    stream.set_write_timeout(Some(Duration::from_secs(30))).ok();

    let mut req = format!("{method} {path} HTTP/1.1\r\nHost: {}\r\nConnection: close\r\n", host());
    if let Some(b) = body {
        req.push_str("Content-Type: application/json\r\n");
        req.push_str(&format!("Content-Length: {}\r\n", b.len()));
        req.push_str("\r\n");
        req.push_str(b);
    } else {
        req.push_str("\r\n");
    }
    stream.write_all(req.as_bytes()).map_err(|e| format!("write: {e}"))?;

    let mut raw = Vec::new();
    stream.read_to_end(&mut raw).map_err(|e| format!("read: {e}"))?;
    let text = String::from_utf8_lossy(&raw).into_owned();
    let (head, body) = match text.split_once("\r\n\r\n") {
        Some((h, b)) => (h, b.to_string()),
        None => (text.as_str(), String::new()),
    };
    let status = head
        .lines()
        .next()
        .and_then(|l| l.split_whitespace().nth(1))
        .and_then(|c| c.parse::<u16>().ok())
        .unwrap_or(0);
    Ok((status, body))
}

fn post(path: &str, body: &str) -> Result<String, String> {
    let (st, b) = http("POST", path, Some(body))?;
    if !(200..300).contains(&st) {
        return Err(format!("WDA POST {path} -> {st}: {}", b.trim()));
    }
    Ok(b)
}
fn get(path: &str) -> Result<String, String> {
    let (st, b) = http("GET", path, None)?;
    if !(200..300).contains(&st) {
        return Err(format!("WDA GET {path} -> {st}: {}", b.trim()));
    }
    Ok(b)
}

// ---------- tiny JSON field extraction (known WDA responses) ----------

fn json_str(body: &str, key: &str) -> Option<String> {
    let needle = format!("\"{key}\"");
    let mut i = body.find(&needle)? + needle.len();
    let bytes = body.as_bytes();
    while i < bytes.len() && bytes[i] != b':' {
        i += 1;
    }
    i += 1;
    while i < bytes.len() && (bytes[i] as char).is_whitespace() {
        i += 1;
    }
    if i >= bytes.len() || bytes[i] != b'"' {
        return None;
    }
    i += 1;
    let mut out = String::new();
    while i < bytes.len() {
        let c = bytes[i] as char;
        if c == '\\' && i + 1 < bytes.len() {
            let n = bytes[i + 1] as char;
            out.push(match n {
                'n' => '\n',
                't' => '\t',
                'r' => '\r',
                other => other,
            });
            i += 2;
            continue;
        }
        if c == '"' {
            break;
        }
        out.push(c);
        i += 1;
    }
    Some(out)
}

fn json_num(body: &str, key: &str) -> Option<f64> {
    let needle = format!("\"{key}\"");
    let mut i = body.find(&needle)? + needle.len();
    let bytes = body.as_bytes();
    while i < bytes.len() && bytes[i] != b':' {
        i += 1;
    }
    i += 1;
    while i < bytes.len() && (bytes[i] as char).is_whitespace() {
        i += 1;
    }
    let start = i;
    while i < bytes.len() {
        let c = bytes[i] as char;
        if c.is_ascii_digit() || c == '.' || c == '-' || c == '+' || c == 'e' || c == 'E' {
            i += 1;
        } else {
            break;
        }
    }
    body[start..i].parse().ok()
}

// ---------- session handling (cached) ----------

fn session_cache_file() -> PathBuf {
    let dir = env::var("MC_STORE_DIR")
        .map(PathBuf::from)
        .or_else(|_| env::var("XDG_CONFIG_HOME").map(|h| PathBuf::from(h).join("mirror-console")))
        .or_else(|_| env::var("HOME").map(|h| PathBuf::from(h).join(".config/mirror-console")))
        .unwrap_or_else(|_| PathBuf::from("."));
    let _ = fs::create_dir_all(&dir);
    dir.join(format!("wda-session-{}-{}", host(), port()))
}

fn cached_session() -> Option<String> {
    fs::read_to_string(session_cache_file()).ok().map(|s| s.trim().to_string()).filter(|s| !s.is_empty())
}

fn create_session() -> Result<String, String> {
    let body = r#"{"capabilities":{"alwaysMatch":{},"firstMatch":[{}]}}"#;
    let resp = post("/session", body)?;
    let sid = json_str(&resp, "sessionId").ok_or("WDA did not return a sessionId")?;
    fs::write(session_cache_file(), &sid).ok();
    Ok(sid)
}

/// Return a working session id, reusing the cached one when it is still valid.
fn session() -> Result<String, String> {
    if let Some(sid) = cached_session() {
        if get(&format!("/session/{sid}/window/size")).is_ok() {
            return Ok(sid);
        }
    }
    create_session()
}

fn window_size(sid: &str) -> Result<(f64, f64), String> {
    let resp = get(&format!("/session/{sid}/window/size"))?;
    let w = json_num(&resp, "width").ok_or("no width in window/size")?;
    let h = json_num(&resp, "height").ok_or("no height in window/size")?;
    Ok((w, h))
}

fn norm(v: &str) -> Result<f64, String> {
    let f: f64 = v.parse().map_err(|_| format!("not a number: {v}"))?;
    Ok(f.clamp(0.0, 1.0))
}

// ---------- commands ----------

pub fn status() -> Result<(), String> {
    let resp = get("/status")?;
    let state = json_str(&resp, "state").unwrap_or_else(|| "reachable".to_string());
    println!("[ok] WDA at {}:{} — {}", host(), port(), state);
    if let Some(ver) = json_str(&resp, "version") {
        println!("     version: {ver}");
    }
    Ok(())
}

pub fn tap(rest: &[String]) -> Result<(), String> {
    let x = norm(rest.first().ok_or("usage: mconsole ios-tap <x0..1> <y0..1>")?)?;
    let y = norm(rest.get(1).ok_or("usage: mconsole ios-tap <x0..1> <y0..1>")?)?;
    let sid = session()?;
    let (w, h) = window_size(&sid)?;
    let body = format!("{{\"x\":{:.1},\"y\":{:.1}}}", x * w, y * h);
    // Appium/WDA 8.x dropped the `/0` (root-element context) suffix — the
    // endpoint is now `/wda/tap` with just x/y in the body. Try that first,
    // fall back to the pre-8 form for older forks.
    if post(&format!("/session/{sid}/wda/tap"), &body).is_err() {
        post(&format!("/session/{sid}/wda/tap/0"), &body)?;
    }
    println!("tap {:.0},{:.0}", x * w, y * h);
    Ok(())
}

pub fn swipe(rest: &[String]) -> Result<(), String> {
    let x1 = norm(rest.first().ok_or(SWIPE_USAGE)?)?;
    let y1 = norm(rest.get(1).ok_or(SWIPE_USAGE)?)?;
    let x2 = norm(rest.get(2).ok_or(SWIPE_USAGE)?)?;
    let y2 = norm(rest.get(3).ok_or(SWIPE_USAGE)?)?;
    let dur: f64 = rest.get(4).map(|s| s.parse().unwrap_or(0.3)).unwrap_or(0.3);
    do_swipe(x1, y1, x2, y2, dur)
}

const SWIPE_USAGE: &str = "usage: mconsole ios-swipe <x1> <y1> <x2> <y2> [duration_s]";

fn do_swipe(x1: f64, y1: f64, x2: f64, y2: f64, dur: f64) -> Result<(), String> {
    let sid = session()?;
    let (w, h) = window_size(&sid)?;
    let body = format!(
        "{{\"fromX\":{:.1},\"fromY\":{:.1},\"toX\":{:.1},\"toY\":{:.1},\"duration\":{:.2}}}",
        x1 * w, y1 * h, x2 * w, y2 * h, dur
    );
    post(&format!("/session/{sid}/wda/dragfromtoforduration"), &body)?;
    println!("swipe {:.0},{:.0} -> {:.0},{:.0}", x1 * w, y1 * h, x2 * w, y2 * h);
    Ok(())
}

pub fn home() -> Result<(), String> {
    post("/wda/homescreen", "{}")?;
    println!("home");
    Ok(())
}

pub fn key(rest: &[String]) -> Result<(), String> {
    let name = rest.first().ok_or("usage: mconsole ios-key <home|back|switcher>")?;
    match name.as_str() {
        "home" => home(),
        // iOS has no hardware back/recent; use the standard system gestures.
        "back" => do_swipe(0.01, 0.5, 0.6, 0.5, 0.25), // edge-swipe from left
        "switcher" | "recent" | "recents" => do_swipe(0.5, 0.999, 0.5, 0.5, 0.3), // swipe up & pause
        other => Err(format!("unknown key `{other}` (home|back|switcher)")),
    }
}

pub fn text(rest: &[String]) -> Result<(), String> {
    let s = rest.first().ok_or("usage: mconsole ios-text <string>")?;
    let sid = session()?;
    // WDA /wda/keys expects {"value":["h","i",...]}
    let chars: Vec<String> = s.chars().map(|c| format!("{:?}", c.to_string())).collect();
    let body = format!("{{\"value\":[{}]}}", chars.join(","));
    post(&format!("/session/{sid}/wda/keys"), &body)?;
    println!("typed {} chars", s.chars().count());
    Ok(())
}

pub fn screenshot(rest: &[String]) -> Result<(), String> {
    let out = rest.first().ok_or("usage: mconsole ios-screenshot <file.png>")?;
    // /screenshot works without a session on WDA.
    let resp = get("/screenshot")?;
    // WDA returns either `{"value":"<b64>"}` (Appium mainline pre-2024) or
    // `{"value":{"screenshot":"<b64>"}}` (mainline + Facebook fork). Try the
    // nested key first, fall back to the flat one.
    let b64 = json_str(&resp, "screenshot")
        .or_else(|| json_str(&resp, "value"))
        .ok_or("no base64 image in response")?;
    let bytes = b64_decode(&b64)?;
    fs::write(out, &bytes).map_err(|e| format!("write {out}: {e}"))?;
    println!("saved {} bytes -> {out}", bytes.len());
    Ok(())
}

// ---------- base64 decode (std-only) ----------

fn b64_decode(s: &str) -> Result<Vec<u8>, String> {
    fn val(c: u8) -> Option<u8> {
        match c {
            b'A'..=b'Z' => Some(c - b'A'),
            b'a'..=b'z' => Some(c - b'a' + 26),
            b'0'..=b'9' => Some(c - b'0' + 52),
            b'+' => Some(62),
            b'/' => Some(63),
            _ => None,
        }
    }
    let mut out = Vec::new();
    let mut buf = 0u32;
    let mut bits = 0u32;
    for &c in s.as_bytes() {
        if c == b'=' || (c as char).is_whitespace() {
            continue;
        }
        let v = val(c).ok_or("invalid base64 character")?;
        buf = (buf << 6) | v as u32;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((buf >> bits) as u8);
        }
    }
    Ok(out)
}
