//! mirror-console — a local-only Android device mirroring & control console.
//!
//! Original, cloud-free tool. It shells out to the open-source `adb` (Android
//! platform-tools, Apache-2.0) and `scrcpy` (Apache-2.0) which the user
//! installs separately. Nothing here talks to any remote server: no accounts,
//! no licensing, no telemetry. A tiny local store keeps per-device notes and
//! stream settings on disk.
//!
//! Binary locations resolve in this order: env override (MC_ADB / MC_SCRCPY),
//! then PATH.

mod ios;

use std::collections::BTreeMap;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode, Stdio};

const VERSION: &str = env!("CARGO_PKG_VERSION");

fn main() -> ExitCode {
    let args: Vec<String> = env::args().skip(1).collect();
    let cmd = args.first().map(String::as_str).unwrap_or("help");
    let rest = &args[args.len().min(1)..];

    let r = match cmd {
        "help" | "-h" | "--help" => {
            print_help();
            Ok(())
        }
        "version" | "-V" | "--version" => {
            println!("mirror-console {VERSION} (local-only, no cloud)");
            Ok(())
        }
        "doctor" => doctor(),
        "devices" => devices(),
        "connect" => connect(rest),
        "disconnect" => disconnect(rest),
        "mirror" => mirror(rest),
        "install" => install(rest),
        "apps" => apps(rest),
        "launch" => app_action(rest, AppAction::Launch),
        "stop" => app_action(rest, AppAction::Stop),
        "screenshot" => screenshot(rest),
        "key" => key(rest),
        "text" => text(rest),
        "ios-status" => ios::status(),
        "ios-tap" => ios::tap(rest),
        "ios-swipe" => ios::swipe(rest),
        "ios-home" => ios::home(),
        "ios-key" => ios::key(rest),
        "ios-text" => ios::text(rest),
        "ios-screenshot" => ios::screenshot(rest),
        "note" => note(rest),
        "set" => set_setting(rest),
        "config" => show_config(),
        other => Err(format!("unknown command: {other}\nrun `mconsole help`")),
    };

    match r {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
    }
}

fn print_help() {
    println!(
        r#"mirror-console {VERSION} — local Android mirroring & control (no cloud)

USAGE
  mconsole <command> [args]

DEVICE
  devices                       list attached devices (adb devices -l)
  connect <host[:port]>         adb-over-Wi-Fi connect (default port 5555)
  disconnect [host[:port]]      disconnect one or all Wi-Fi devices
  doctor                        check that adb / scrcpy are available

MIRROR
  mirror [serial]               start scrcpy mirror (uses stored stream settings)

APPS
  apps [serial]                 list installed third-party packages
  install <serial|-> <apk>      install an APK
  launch <serial|-> <pkg>       launch an app
  stop <serial|-> <pkg>         force-stop an app

INPUT / MEDIA
  key <serial|-> <name>         send key: home|back|recent|power|volup|voldown
  text <serial|-> <string>      type text on the device
  screenshot <serial|-> <file>  save a PNG screenshot from the device

IOS (via open-source WebDriverAgent; forward device port 8100 to localhost)
  ios-status                    check the WDA endpoint is reachable
  ios-tap <x> <y>               tap at normalized 0..1 coordinates
  ios-swipe <x1> <y1> <x2> <y2> [dur]   swipe/drag between normalized points
  ios-home                      go to the home screen
  ios-key <home|back|switcher>  system gestures (back = edge swipe)
  ios-text <string>             type text into the focused field
  ios-screenshot <file>         save a PNG screenshot
  (set MC_WDA_HOST / MC_WDA_PORT to point at your WDA tunnel)

LOCAL STORE (on-disk, no network)
  note <serial> <text...>       attach a note to a device
  set <key> <value>             stream setting: video_max_size|video_bit_rate|video_fps
  config                        print stored settings and notes

Use "-" as <serial> to target the only/default device.
Binaries: set MC_ADB / MC_SCRCPY to override discovery on PATH.
"#
    );
}

// ---------- external tool discovery ----------

fn tool(env_key: &str, name: &str) -> Result<String, String> {
    if let Ok(p) = env::var(env_key) {
        if !p.trim().is_empty() {
            return Ok(p);
        }
    }
    // Rely on PATH resolution by the OS; just return the bare name.
    // A friendly existence check happens in `doctor`.
    Ok(name.to_string())
}

fn adb() -> Result<String, String> {
    tool("MC_ADB", "adb")
}

fn scrcpy() -> Result<String, String> {
    tool("MC_SCRCPY", "scrcpy")
}

fn run(bin: &str, args: &[&str]) -> Result<String, String> {
    let out = Command::new(bin)
        .args(args)
        .stdin(Stdio::null())
        .output()
        .map_err(|e| format!("failed to run `{bin}`: {e} (is it installed / on PATH?)"))?;
    if !out.status.success() {
        let err = String::from_utf8_lossy(&out.stderr);
        return Err(format!(
            "`{bin} {}` exited with {}: {}",
            args.join(" "),
            out.status,
            err.trim()
        ));
    }
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

/// Run interactively, inheriting stdio (for scrcpy which is long-lived).
fn spawn_inherit(bin: &str, args: &[&str]) -> Result<(), String> {
    let status = Command::new(bin)
        .args(args)
        .status()
        .map_err(|e| format!("failed to launch `{bin}`: {e} (is it installed / on PATH?)"))?;
    if !status.success() {
        return Err(format!("`{bin}` exited with {status}"));
    }
    Ok(())
}

// ---------- serial resolution ----------

fn list_serials() -> Result<Vec<String>, String> {
    let out = run(&adb()?, &["devices"])?;
    let mut v = Vec::new();
    for line in out.lines().skip(1) {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let mut it = line.split_whitespace();
        if let (Some(serial), Some(state)) = (it.next(), it.next()) {
            if state == "device" {
                v.push(serial.to_string());
            }
        }
    }
    Ok(v)
}

/// Resolve the target serial. `arg` may be an explicit serial, "-" for the
/// default device, or None (same as "-").
fn resolve_serial(arg: Option<&String>) -> Result<String, String> {
    match arg.map(String::as_str) {
        Some(s) if s != "-" => Ok(s.to_string()),
        _ => {
            let serials = list_serials()?;
            match serials.len() {
                0 => Err("no devices in state `device` — connect one or run `mconsole devices`".into()),
                1 => Ok(serials[0].clone()),
                _ => Err(format!(
                    "multiple devices attached, specify one explicitly: {}",
                    serials.join(", ")
                )),
            }
        }
    }
}

fn adb_dev(serial: &str, args: &[&str]) -> Result<String, String> {
    let mut full = vec!["-s", serial];
    full.extend_from_slice(args);
    run(&adb()?, &full)
}

// ---------- commands ----------

fn doctor() -> Result<(), String> {
    let mut ok = true;
    for (env_key, name) in [("MC_ADB", "adb"), ("MC_SCRCPY", "scrcpy")] {
        let bin = tool(env_key, name)?;
        match Command::new(&bin).arg("--version").stdout(Stdio::piped()).stderr(Stdio::piped()).output() {
            Ok(o) => {
                let v = String::from_utf8_lossy(&o.stdout);
                let first = v.lines().next().unwrap_or("").trim();
                println!("[ok]   {name}: {}", if first.is_empty() { bin.as_str() } else { first });
            }
            Err(_) => {
                ok = false;
                println!("[MISS] {name}: not found (set {env_key} or install it)");
            }
        }
    }
    println!("[info] store: {}", store_dir().display());
    if ok {
        Ok(())
    } else {
        Err("one or more required tools are missing".into())
    }
}

fn devices() -> Result<(), String> {
    print!("{}", run(&adb()?, &["devices", "-l"])?);
    Ok(())
}

fn connect(rest: &[String]) -> Result<(), String> {
    let host = rest.first().ok_or("usage: mconsole connect <host[:port]>")?;
    let target = if host.contains(':') {
        host.clone()
    } else {
        format!("{host}:5555")
    };
    print!("{}", run(&adb()?, &["connect", &target])?);
    // remember it in the local store
    let mut cfg = Store::load();
    cfg.wifi_endpoints.insert(target.clone(), now_secs());
    cfg.save()?;
    Ok(())
}

fn disconnect(rest: &[String]) -> Result<(), String> {
    match rest.first() {
        Some(host) => {
            let target = if host.contains(':') { host.clone() } else { format!("{host}:5555") };
            print!("{}", run(&adb()?, &["disconnect", &target])?);
            let mut cfg = Store::load();
            cfg.wifi_endpoints.remove(&target);
            cfg.save()?;
        }
        None => {
            print!("{}", run(&adb()?, &["disconnect"])?);
        }
    }
    Ok(())
}

fn mirror(rest: &[String]) -> Result<(), String> {
    let serial = resolve_serial(rest.first())?;
    let cfg = Store::load();
    let mut args: Vec<String> = vec!["-s".into(), serial.clone()];
    if let Some(v) = cfg.settings.get("video_max_size") {
        args.push("--max-size".into());
        args.push(v.clone());
    }
    if let Some(v) = cfg.settings.get("video_bit_rate") {
        args.push("--video-bit-rate".into());
        args.push(v.clone());
    }
    if let Some(v) = cfg.settings.get("video_fps") {
        args.push("--max-fps".into());
        args.push(v.clone());
    }
    let refs: Vec<&str> = args.iter().map(String::as_str).collect();
    println!("[info] launching scrcpy for {serial} (close the window to stop)");
    spawn_inherit(&scrcpy()?, &refs)
}

fn install(rest: &[String]) -> Result<(), String> {
    let serial = resolve_serial(rest.first())?;
    let apk = rest.get(1).ok_or("usage: mconsole install <serial|-> <apk>")?;
    if !Path::new(apk).exists() {
        return Err(format!("apk not found: {apk}"));
    }
    print!("{}", adb_dev(&serial, &["install", "-r", apk])?);
    Ok(())
}

fn apps(rest: &[String]) -> Result<(), String> {
    let serial = resolve_serial(rest.first())?;
    let out = adb_dev(&serial, &["shell", "pm", "list", "packages", "-3"])?;
    let mut pkgs: Vec<&str> = out
        .lines()
        .filter_map(|l| l.strip_prefix("package:"))
        .map(str::trim)
        .collect();
    pkgs.sort_unstable();
    for p in pkgs {
        println!("{p}");
    }
    Ok(())
}

enum AppAction {
    Launch,
    Stop,
}

fn app_action(rest: &[String], action: AppAction) -> Result<(), String> {
    let serial = resolve_serial(rest.first())?;
    let pkg = rest.get(1).ok_or("usage: mconsole <launch|stop> <serial|-> <package>")?;
    match action {
        AppAction::Launch => {
            print!(
                "{}",
                adb_dev(&serial, &["shell", "monkey", "-p", pkg, "-c", "android.intent.category.LAUNCHER", "1"])?
            );
        }
        AppAction::Stop => {
            adb_dev(&serial, &["shell", "am", "force-stop", pkg])?;
            println!("stopped {pkg}");
        }
    }
    Ok(())
}

fn screenshot(rest: &[String]) -> Result<(), String> {
    let serial = resolve_serial(rest.first())?;
    let out = rest.get(1).ok_or("usage: mconsole screenshot <serial|-> <file.png>")?;
    // exec-out streams raw PNG bytes to stdout; capture them directly.
    let bin = adb()?;
    let output = Command::new(&bin)
        .args(["-s", &serial, "exec-out", "screencap", "-p"])
        .stdin(Stdio::null())
        .output()
        .map_err(|e| format!("failed to run adb: {e}"))?;
    if !output.status.success() {
        return Err(format!("screencap failed: {}", String::from_utf8_lossy(&output.stderr).trim()));
    }
    fs::write(out, &output.stdout).map_err(|e| format!("write {out}: {e}"))?;
    println!("saved {} bytes -> {out}", output.stdout.len());
    Ok(())
}

fn key(rest: &[String]) -> Result<(), String> {
    let serial = resolve_serial(rest.first())?;
    let name = rest.get(1).ok_or("usage: mconsole key <serial|-> <name>")?;
    let code = match name.as_str() {
        "home" => "KEYCODE_HOME",
        "back" => "KEYCODE_BACK",
        "recent" | "recents" => "KEYCODE_APP_SWITCH",
        "power" => "KEYCODE_POWER",
        "volup" => "KEYCODE_VOLUME_UP",
        "voldown" => "KEYCODE_VOLUME_DOWN",
        "menu" => "KEYCODE_MENU",
        "enter" => "KEYCODE_ENTER",
        other => return Err(format!("unknown key `{other}` (home|back|recent|power|volup|voldown|menu|enter)")),
    };
    adb_dev(&serial, &["shell", "input", "keyevent", code])?;
    println!("sent {name}");
    Ok(())
}

fn text(rest: &[String]) -> Result<(), String> {
    let serial = resolve_serial(rest.first())?;
    let s = rest.get(1).ok_or("usage: mconsole text <serial|-> <string>")?;
    // adb input text uses %s for spaces and does not accept some chars.
    let escaped = s.replace(' ', "%s");
    adb_dev(&serial, &["shell", "input", "text", &escaped])?;
    println!("typed {} chars", s.len());
    Ok(())
}

fn note(rest: &[String]) -> Result<(), String> {
    let serial = rest.first().ok_or("usage: mconsole note <serial> <text...>")?;
    if rest.len() < 2 {
        return Err("usage: mconsole note <serial> <text...>".into());
    }
    let text = rest[1..].join(" ");
    let mut cfg = Store::load();
    cfg.notes.insert(serial.clone(), text);
    cfg.save()?;
    println!("note saved for {serial}");
    Ok(())
}

fn set_setting(rest: &[String]) -> Result<(), String> {
    let key = rest.first().ok_or("usage: mconsole set <key> <value>")?;
    let val = rest.get(1).ok_or("usage: mconsole set <key> <value>")?;
    match key.as_str() {
        "video_max_size" | "video_bit_rate" | "video_fps" => {}
        other => return Err(format!("unknown setting `{other}` (video_max_size|video_bit_rate|video_fps)")),
    }
    let mut cfg = Store::load();
    cfg.settings.insert(key.clone(), val.clone());
    cfg.save()?;
    println!("{key} = {val}");
    Ok(())
}

fn show_config() -> Result<(), String> {
    let cfg = Store::load();
    println!("# store: {}", store_file().display());
    println!("[settings]");
    for (k, v) in &cfg.settings {
        println!("{k} = {v}");
    }
    println!("[notes]");
    for (k, v) in &cfg.notes {
        println!("{k}: {v}");
    }
    println!("[wifi_endpoints]");
    for (k, v) in &cfg.wifi_endpoints {
        println!("{k} (last_ok={v})");
    }
    Ok(())
}

// ---------- local store (no network) ----------
//
// Minimal, dependency-free line store:
//   S<TAB>key<TAB>value      settings
//   N<TAB>serial<TAB>text    device note
//   W<TAB>host:port<TAB>ts   wifi endpoint
// Values must not contain tabs or newlines; we sanitize on write.

struct Store {
    settings: BTreeMap<String, String>,
    notes: BTreeMap<String, String>,
    wifi_endpoints: BTreeMap<String, u64>,
}

impl Store {
    fn load() -> Self {
        let mut s = Store {
            settings: BTreeMap::new(),
            notes: BTreeMap::new(),
            wifi_endpoints: BTreeMap::new(),
        };
        if let Ok(txt) = fs::read_to_string(store_file()) {
            for line in txt.lines() {
                let mut it = line.splitn(3, '\t');
                match (it.next(), it.next(), it.next()) {
                    (Some("S"), Some(k), Some(v)) => {
                        s.settings.insert(k.to_string(), v.to_string());
                    }
                    (Some("N"), Some(k), Some(v)) => {
                        s.notes.insert(k.to_string(), v.to_string());
                    }
                    (Some("W"), Some(k), Some(v)) => {
                        s.wifi_endpoints.insert(k.to_string(), v.parse().unwrap_or(0));
                    }
                    _ => {}
                }
            }
        }
        s
    }

    fn save(&self) -> Result<(), String> {
        let dir = store_dir();
        fs::create_dir_all(&dir).map_err(|e| format!("create {}: {e}", dir.display()))?;
        let mut out = String::new();
        for (k, v) in &self.settings {
            out.push_str(&format!("S\t{}\t{}\n", clean(k), clean(v)));
        }
        for (k, v) in &self.notes {
            out.push_str(&format!("N\t{}\t{}\n", clean(k), clean(v)));
        }
        for (k, v) in &self.wifi_endpoints {
            out.push_str(&format!("W\t{}\t{}\n", clean(k), v));
        }
        fs::write(store_file(), out).map_err(|e| format!("write store: {e}"))
    }
}

fn clean(s: &str) -> String {
    s.chars().map(|c| if c == '\t' || c == '\n' { ' ' } else { c }).collect()
}

fn store_dir() -> PathBuf {
    if let Ok(d) = env::var("MC_STORE_DIR") {
        return PathBuf::from(d);
    }
    let base = env::var("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|_| env::var("HOME").map(|h| Path::new(&h).join(".config")))
        .or_else(|_| env::var("APPDATA").map(PathBuf::from))
        .unwrap_or_else(|_| PathBuf::from("."));
    base.join("mirror-console")
}

fn store_file() -> PathBuf {
    store_dir().join("store.tsv")
}

fn now_secs() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}
