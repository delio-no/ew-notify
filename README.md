# EndlessWar Notifier

Уведомления о событиях в игре EndlessWar в реальном времени.

## Описание

EndlessWar Notifier — это десктопное приложение для Windows и Linux, которое анализирует сетевые пакеты процесса игры `EndlessWar.exe` и отправляет уведомления при обнаружении заданных байтовых последовательностей.

### Как это работает

1. Приложение запускается и отображает GUI-интерфейс
2. Пользователь запускает игру EndlessWar
3. Приложение перехватывает сетевые пакеты игры через WinDivert (Windows)
4. При обнаружении заданной байтовой последовательности — отправляется уведомление
5. Уведомления приходят в Telegram-бот и в системный трэй Windows

### Возможности

- **Анализ сетевых пакетов**: Перехват и анализ трафика процесса EndlessWar.exe через WinDivert (Windows)
- **Поиск по сигнатурам**: Обнаружение заданных байтовых последовательностей в пакетах
- **Telegram-уведомления**: Отправка мгновенных уведомлений через Telegram-бот
- **Telegram-бот**: Бот с командами `/start`, `/myid`, `/users` для регистрации пользователей
- **Управление пользователями**: Включение/отключение пользователей для рассылки уведомлений
- **Windows Tray**: Уведомления в системном трэе Windows
- **GUI-интерфейс**: Удобное управление и мониторинг через десктопное приложение
- **Настраиваемые правила**: Гибкая настройка паттернов для поиска
- **Кулдаун**: Настраиваемый интервал между уведомлениями

## Технический стэк

- **Язык**: Rust
- **GUI**: [Tauri](https://tauri.app/) — фреймворк для создания десктопных приложений на Rust + WebView
- **Перехват пакетов**: [WinDivert](https://reqrypt.org/windivert.html) — перехват сетевых пакетов в Windows (только Windows)
- **Telegram-бот**: [teloxide](https://github.com/teloxide/teloxide) — библиотека для создания Telegram-ботов на Rust
- **Системный трэй**: [tauri-plugin-notification](https://github.com/tauri-apps/plugin-notification) + системные уведомления Windows

### Зависимости

- Windows 10+ или Linux
- Rust 1.70+
- WinDivert driver — устанавливается при первом запуске (только Windows)

## Установка

### Требования

- [Rust](https://rustup.rs/) — установлен через rustup
- [WebView2](https://developer.microsoft.com/en-us/microsoft-edge/webview2/) — для Tauri GUI (Windows)
- WinDivert driver — устанавливается при первом запуске (только Windows)
- WebKit (Linux) — обычно уже установлен в современных дистрибутивах

### Сборка из исходников

```bash
git clone https://github.com/yourusername/ew_notify.git
cd ew_notify
npm install
cargo tauri build
```

### Запуск

```bash
npm run tauri dev    # Режим разработки
npm run tauri build # Сборка релиз-версии
```

## Настройка

### Файл конфигурации

При первом запуске создаётся файл `config.json`:

```json
{
  "telegram": {
    "bot_token": "your_bot_token",
    "auto_start": false,
    "registered_users": [],
    "notify_all_users": true
  },
  "target": {
    "process_name": "EndlessWar.exe",
    "patterns": [
      {
        "name": "event_started",
        "bytes": "A1B2C3D4",
        "description": "Событие началось"
      },
      {
        "name": "tournament",
        "bytes": "FF000100",
        "description": "Турнир"
      }
    ]
  },
  "notifications": {
    "telegram_enabled": true,
    "tray_enabled": true,
    "cooldown_seconds": 60
  }
}
```

### Настройка Telegram-бота

1. Создайте бота через @BotFather в Telegram
2. Получите токен бота
3. Укажите токен в конфигурации и запустите бота

#### Команды бота

- `/start` — Зарегистрироваться в боте
- `/myid` — Узнать свой Telegram ID
- `/users` — Список пользователей (только для админа)

Администратор может управлять пользователями через GUI-интерфейс приложения.

## Использование

### Запуск мониторинга

1. Запустите приложение
2. Введите имя процесса (по умолчанию: EndlessWar.exe)
3. Нажмите "Начать мониторинг"
4. Запустите игру EndlessWar

### Добавление паттернов

1. Перейдите на вкладку "Паттерны"
2. Введите hex-строку (например: `A1B2C3D4` или `A1 B2 C3 D4`)
3. Введите описание
4. Нажмите "Добавить паттерн"

### Использование Telegram-бота

1. Введите токен бота в настройках
2. Нажмите "Запустить бота"
3. Пользователи отправляют `/start` боту для регистрации
4. В разделе "Пользователи" админ может включать/отключать пользователей для рассылки

### Системный трэй

Приложение отображается в системном трэе:
- Правый клик — меню (Старт мониторинга, Стоп мониторинга, Показать, Выход)
- Левая кнопка — показать/скрыть окно

## Структура проекта

```
ew_notify/
├── Cargo.toml
├── package.json
├── config.json              # Конфигурация по умолчанию
├── src/                      # Frontend (Svelte + TypeScript)
│   ├── main.ts
│   ├── App.svelte
│   └── styles.css
├── src-tauri/
│   ├── Cargo.toml
│   ├── tauri.conf.json
│   ├── build.rs
│   ├── gen/
│   ├── icons/
│   └── src/
│       ├── main.rs           # Точка входа Tauri
│       ├── lib.rs            # Библиотека
│       ├── packet/
│       │   └── mod.rs        # Перехват пакетов (Windows)
│       ├── telegram/
│       │   ├── mod.rs        # Telegram-модуль
│       │   └── client.rs
│       ├── tray.rs           # Системный трэй
│       └── config.rs         # Конфигурация
├── dist/                     # Скомпилированный frontend
└── target/                   # Cargo build output
```

## Сборка

### Windows

```bash
cargo build --release
cargo tauri build
```

### Linux

Для сборки на Linux потребуются дополнительные системные зависимости:

**Ubuntu/Debian:**
```bash
sudo apt install libwebkit2gtk-4.1-dev libappindicator3-dev librsvg2-dev patchelf
```

**Fedora:**
```bash
sudo dnf install webkit2gtk4.1-devel libappindicator-gtk3-devel
```

**Arch Linux:**
```bash
sudo pacman -S webkit2gtk-4.1 libappindicator
```

```bash
cargo build --release
cargo tauri build --target x86_64-unknown-linux-gnu
```

Примечание: На Linux мониторинг пакетов недоступен (требуется WinDivert), но Telegram-бот работает.

### Запуск в режиме разработки

```bash
cargo tauri dev    # Режим разработки
```

## Архитектура

```
┌─────────────────────────────────────────────────────────┐
│                    EndlessWar Notifier                   │
├─────────────────────────────────────────────────────────┤
│  ┌─────────────┐    ┌─────────────┐    ┌─────────────┐│
│  │  Tauri GUI  │───▶│  Analyzer   │───▶│   Telegram   ││
│  │  (WebView)  │    │  (patterns) │    │  (teloxide)  ││
│  └─────────────┘    └─────────────┘    └─────────────┘│
│         │                  │                  │         │
│         ▼                  ▼                  ▼         │
│  ┌─────────────────────────────────────────────────────┐│
│  │              WinDivert (packet capture)             ││
│  │                    (только Windows)                  ││
│  └─────────────────────────────────────────────────────┘│
│         │                                                │
│         ▼                                                │
│  ┌─────────────────────────────────────────────────────┐│
│  │              EndlessWar.exe (process)               ││
│  └─────────────────────────────────────────────────────┘│
└─────────────────────────────────────────────────────────┘
                            │
                            ▼
               ┌─────────────────────────┐
               │   Windows Tray Icon      │
               │   (системные уведомления)│
               └─────────────────────────┘
```

## Лицензия

MIT License

## Ссылки

- [EndlessWar](https://endlesswar.live) — официальный сайт игры
- [Tauri](https://tauri.app/) — документация фреймворка
- [teloxide](https://github.com/teloxide/teloxide) — документация Telegram-бота
- [WinDivert](https://reqrypt.org/windivert.html) — документация перехвата пакетов
