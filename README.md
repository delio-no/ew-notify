# EndlessWar Notifier

Уведомления о событиях в игре EndlessWar в реальном времени.

## Описание

EndlessWar Notifier — это локальное десктопное приложение для Windows, которое анализирует сетевые пакеты процесса игры `EndlessWar.exe` и отправляет уведомления при обнаружении заданных байтовых последовательностей.

### Как это работает

1. Приложение запускается и отображает GUI-интерфейс
2. Пользователь запускает игру EndlessWar
3. Приложение перехватывает сетевые пакеты игры через WinDivert
4. При обнаружении заданной байтовой последовательности — отправляется уведомление
5. Уведомления приходят в Telegram-бот и в системный трэй Windows

### Возможности

- **Анализ сетевых пакетов**: Перехват и анализ трафика процесса EndlessWar.exe через WinDivert
- **Поиск по сигнатурам**: Обнаружение заданных байтовых последовательностей в пакетах
- **Telegram-уведомления**: Отправка мгновенных уведомлений через Telegram-бот
- **Windows Tray**: Уведомления в системном трэе Windows
- **GUI-интерфейс**: Удобное управление и мониторинг через десктопное приложение
- **Настраиваемые правила**: Гибкая настройка паттернов для поиска

## Технический стэк

- **Язык**: Rust
- **GUI**: [Tauri](https://tauri.app/) — фреймворк для создания десктопных приложений на Rust + WebView
- **Перехват пакетов**: [WinDivert](https://reqrypt.org/windivert.html) — перехват сетевых пакетов в Windows
- **Telegram-бот**: [teloxide](https://github.com/teloxide/teloxide) — библиотека для создания Telegram-ботов на Rust
- **Системный трэй**: [tauri-plugin-notification](https://github.com/tauri-apps/plugin-notification) + системные уведомления Windows

### Зависимости

- Windows 10+
- Rust 1.70+
- WinDivert (драйвер устанавливается вместе с приложением)

## Установка

### Требования

- [Rust](https://rustup.rs/) — установлен через rustup
- [WebView2](https://developer.microsoft.com/en-us/microsoft-edge/webview2/) — для Tauri GUI
- WinDivert driver — устанавливается при первом запуске

### Сборка из исходников

```bash
git clone https://github.com/yourusername/ew_notify.git
cd ew_notify
cargo build --release
```

### Запуск

```bash
cargo tauri dev    # Режим разработки
cargo tauri build # Сборка релиз-версии
```

## Настройка

### Файл конфигурации

При первом запуске создаётся файл `config.json`:

```json
{
  "telegram": {
    "bot_token": "your_bot_token",
    "chat_id": "123456789"
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
3. Укажите токен и chat_id в конфигурации

## Использование

### Запуск мониторинга

1. Запустите приложение
2. Введите имя процесса (по умолчанию: EndlessWar.exe)
3. Нажмите "Начать мониторинг"
4. Запустите игру EndlessWar

### Добавление паттернов

1. Введите hex-строку (например: `A1B2C3D4` или `A1 B2 C3 D4`)
2. Введите описание
3. Нажмите "Добавить паттерн"

### Системный трэй

Приложение отображается в системном трэе:
- Правый клик — меню (Свернуть, Выход)
- Левая кнопка — показать/скрыть окно

## Структура проекта

```
ew_notify/
├── Cargo.toml
├── src-tauri/
│   ├── Cargo.toml
│   ├── src/
│   │   ├── main.rs           # Точка входа Tauri
│   │   ├── lib.rs           # Библиотека
│   │   ├── packet/
│   │   │   ├── mod.rs       # Перехват пакетов
│   │   │   └── analyzer.rs  # Анализ паттернов
│   │   ├── telegram/
│   │   │   ├── mod.rs       # Telegram-модуль
│   │   │   └── client.rs    # Teloxide клиент
│   │   ├── tray.rs          # Системный трэй
│   │   └── config.rs        # Конфигурация
│   ├── tauri.conf.json
│   └── icons/
├── src/                      # Frontend (WebView)
│   ├── main.ts
│   ├── App.svelte
│   └── styles.css
├── config.json              # Конфигурация по умолчанию
└── package.json
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
