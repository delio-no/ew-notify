# EndlessWar Notifier

Уведомления о событиях в игре EndlessWar в реальном времени.

## Описание

Локальное Windows-приложение (Tauri 2 + Svelte), которое перехватывает inbound-трафик игры через WinDivert, ищет байтовые паттерны и шлёт уведомления в Telegram и системный трей.

### Как это работает

1. Запускаешь приложение (нужны права администратора).
2. Режим **авто**: ждём `EndlessWar.exe`, берём remote IP/port из TCP-таблицы процесса.
3. Режим **ручной**: фильтр по порту/IP из настроек (по умолчанию TCP `7777`).
4. При совпадении паттерна — уведомление в трей и Telegram.

### Возможности

- Автообнаружение remote endpoint процесса или ручной фильтр
- Паттерны DEATH / STOP_FARM из коробки, экспорт/импорт, тест hex
- Telegram-бот (`/start`, `/myid`, `/users` только для admin)
- Трей, автозапуск мониторинга, свернуть в трей
- Консоль логов и опциональный дамп пакетов

## Стек

- Rust + [Tauri 2](https://tauri.app/)
- [WinDivert](https://reqrypt.org/windivert.html)
- [teloxide](https://github.com/teloxide/teloxide)

## Требования

- Windows 10+
- Rust, Node.js, WebView2
- Права администратора (манифест `requireAdministrator`)
- Рядом с exe: `WinDivert.dll`, `WinDivert64.sys` / `WinDivert32.sys`

## Сборка и запуск

```bash
npm install
npm run tauri dev     # разработка
npm run tauri build   # NSIS-инсталлятор + exe
```

При `tauri build` WinDivert-файлы кладутся рядом с приложением (bundle resources). При `cargo`/`tauri dev` их копирует `build.rs` в `target/...`.

**Важно:**
- Один экземпляр (mutex).
- `config.json` и `app.log` — **рядом с exe** (или `EW_NOTIFY_CONFIG` для конфига).
- Не коммить токен бота.

## Конфигурация

Пример `config.json`:

```json
{
  "telegram": {
    "bot_token": "",
    "admin_chat_id": "",
    "auto_start": false,
    "registered_users": [],
    "notify_all_users": true
  },
  "target": {
    "process_name": "EndlessWar.exe",
    "capture_mode": "auto",
    "tcp_port": 7777,
    "udp_port": null,
    "remote_ip": null,
    "patterns": [
      {
        "name": "DEATH",
        "bytes": "00FE06032C010000",
        "description": "Персонаж умер"
      },
      {
        "name": "STOP_FARM",
        "bytes": "4E00FE11006100750074006F006600610072006D000000",
        "description": "Автофарм остановлен"
      }
    ]
  },
  "notifications": {
    "telegram_enabled": true,
    "tray_enabled": true,
    "cooldown_seconds": 60,
    "burst_dedup_ms": 2000
  },
  "startup": {
    "auto_start_monitoring": false,
    "start_minimized_to_tray": false,
    "close_to_tray": true
  }
}
```

### Telegram

1. Создай бота у @BotFather, вставь токен в настройки.
2. Напиши боту `/start` (регистрация).
3. `/myid` — узнать свой ID.
4. Укажи `admin_chat_id` (свой ID) — только он может `/users`.

## Использование

1. Настройки → режим захвата (авто/ручной), Telegram, трей.
2. Дашборд → «Начать мониторинг».
3. Паттерны — добавить / импорт / тест hex; из консоли пакетов можно создать паттерн из dump.

## Лицензия

MIT License

## Ссылки

- [EndlessWar](https://endlesswar.live)
- [Tauri](https://tauri.app/)
- [teloxide](https://github.com/teloxide/teloxide)
- [WinDivert](https://reqrypt.org/windivert.html)
