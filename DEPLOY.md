# mirror-console — развёртывание

Локальная утилита зеркалирования и управления телефонами:
- **Android** — через open-source `adb` + `scrcpy`
- **iPhone** — через open-source `WebDriverAgent`

Без облака, без лицензий, без телеметрии. Всё работает локально.

Код уже загружен на GitHub:
- Репозиторий: https://github.com/NikitoriyPennison/Avi
- Ветка: `claude/app-analysis-decompilation-q87e2w`
- Папка: `mirror-console/`

---

## Вариант A — развернуть через Claude Code на Mac (рекомендуется)

Открой Claude Code в терминале на своём Mac (там, где подключены телефоны) и
вставь текст между линиями ниже.

------------------------------------------------------------------------
Разверни проект mirror-console из моего репозитория на этом Mac.

Репозиторий: https://github.com/NikitoriyPennison/Avi
Ветка: claude/app-analysis-decompilation-q87e2w
Папка проекта: mirror-console/

Это локальная утилита зеркалирования и управления телефонами (Android через
adb+scrcpy, iPhone через open-source WebDriverAgent). Без облака и лицензий.

Сделай по шагам и после каждого показывай результат:

1) Клонируй репозиторий (если ещё нет) и переключись на ветку
   claude/app-analysis-decompilation-q87e2w. Перейди в папку mirror-console.

2) Установи зависимости через Homebrew (если чего-то нет):
   brew install rust scrcpy android-platform-tools go-ios
   Проверь, что установлен Xcode: xcode-select -p

3) Собери десктоп-приложение под macOS:
   ./build-macos.sh
   Затем: ./dist/mconsole doctor    (adb и scrcpy должны быть [ok])

4) Проверь Android: подключи телефон с USB-отладкой и выполни
   ./dist/mconsole devices
   ./dist/mconsole mirror -

5) Для iPhone собери агента WebDriverAgent в .ipa. Спроси у меня мой
   Apple Team ID (10 символов) и выполни:
   ./build-wda-ipa-macos.sh <APPLE_TEAM_ID>
   Установи и запусти агента на подключённом iPhone:
   go-ios install --path=dist/WebDriverAgentRunner.ipa --udid <UDID>
   go-ios runwda --udid <UDID>
   в отдельном процессе: go-ios forward 8100 8100 --udid <UDID>

6) Проверь управление iPhone:
   ./dist/mconsole ios-status
   ./dist/mconsole ios-home
   ./dist/mconsole ios-tap 0.5 0.5
   ./dist/mconsole ios-screenshot iphone.png

Если что-то не собирается или устройство не видно — разберись по логам и
исправь. README и все скрипты уже в папке mirror-console.
------------------------------------------------------------------------

---

## Вариант B — вручную в терминале Mac

```bash
# 1. Получить код
git clone https://github.com/NikitoriyPennison/Avi.git
cd Avi
git checkout claude/app-analysis-decompilation-q87e2w
cd mirror-console

# 2. Зависимости
brew install rust scrcpy android-platform-tools go-ios
xcode-select -p          # должен быть установлен Xcode

# 3. Сборка десктоп-приложения (universal arm64+x86_64)
./build-macos.sh
./dist/mconsole doctor

# 4. Android
./dist/mconsole devices
./dist/mconsole mirror -

# 5. iPhone: собрать и установить агента
./build-wda-ipa-macos.sh <APPLE_TEAM_ID>
go-ios install --path=dist/WebDriverAgentRunner.ipa --udid <UDID>
go-ios runwda --udid <UDID>
# в отдельном окне:
go-ios forward 8100 8100 --udid <UDID>

# 6. Управление iPhone
./dist/mconsole ios-status
./dist/mconsole ios-home
./dist/mconsole ios-tap 0.5 0.5
./dist/mconsole ios-screenshot iphone.png
```

---

## Вариант C — сборка в облаке GitHub Actions (без Mac у тебя)

В репозитории уже есть workflow `.github/workflows/macos-build.yml`:

- **macOS-бинарь** собирается автоматически при каждом пуше →
  скачивается из артефактов запуска (`mconsole-macos-universal`).
- **iPhone .ipa** — вкладка Actions → *macOS build* → *Run workflow* →
  `build_ipa = true`. Предварительно добавь секреты
  (Settings → Secrets and variables → Actions):
  - `APPLE_TEAM_ID` — Team ID (10 символов)
  - `ASC_KEY_ID`, `ASC_ISSUER_ID` — id и issuer ключа App Store Connect API
  - `ASC_API_KEY_BASE64` — файл .p8 в base64: `base64 -i AuthKey_XXXX.p8`

Готовый `.ipa` появится в артефактах запуска (`WebDriverAgentRunner-ipa`).

---

## Команды mconsole (шпаргалка)

Android:
```
mconsole devices                 список устройств
mconsole mirror [serial|-]       зеркалирование (scrcpy)
mconsole connect <host[:port]>   adb-over-Wi-Fi
mconsole apps -                  список сторонних пакетов
mconsole install - app.apk       установить APK
mconsole launch - <pkg>          запустить приложение
mconsole stop - <pkg>            остановить приложение
mconsole key - home              home|back|recent|power|volup|voldown|menu|enter
mconsole text - "текст"          ввод текста
mconsole screenshot - shot.png   скриншот
```

iPhone (после запуска WDA и forward 8100):
```
mconsole ios-status              проверка связи с WDA
mconsole ios-tap <x> <y>         тап (координаты 0..1)
mconsole ios-swipe <x1> <y1> <x2> <y2> [сек]   свайп
mconsole ios-home                домой
mconsole ios-key <home|back|switcher>          жесты
mconsole ios-text "текст"        ввод текста
mconsole ios-screenshot out.png  скриншот
```

Переменные окружения: `MC_ADB`, `MC_SCRCPY` (пути к инструментам),
`MC_WDA_HOST`, `MC_WDA_PORT` (адрес WDA-туннеля, по умолчанию 127.0.0.1:8100).

---

## Важные оговорки

- Сборку `.ipa` и macOS-бинаря можно делать **только на Mac** (требование Apple:
  нужен Xcode и подпись твоим Apple ID / Team). Это же ограничение было у
  оригинала — он собирал агент на машине пользователя.
- iPhone-агент — это open-source WebDriverAgent (Appium), а не чужой
  проприетарный tap-агент.
- Всё локальное: ни одного обращения к внешним серверам.
