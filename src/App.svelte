<script lang="ts">
  import { invoke } from '@tauri-apps/api/core';
  import { listen } from '@tauri-apps/api/event';
  import { onMount } from 'svelte';

  interface Pattern {
    name: string;
    bytes: string;
    description: string;
  }

  interface RegisteredUser {
    chat_id: string;
    username: string | null;
    first_name: string | null;
    enabled: boolean;
  }

  interface Config {
    telegram: {
      bot_token: string;
      auto_start: boolean;
      registered_users: RegisteredUser[];
      notify_all_users: boolean;
    };
    target: {
      process_name: string;
      patterns: Pattern[];
    };
    notifications: {
      telegram_enabled: boolean;
      tray_enabled: boolean;
      cooldown_seconds: number;
    };
  }

  let currentPage = 'dashboard';
  let config: Config = {
    telegram: { bot_token: '', auto_start: false, registered_users: [], notify_all_users: true },
    target: { process_name: 'EndlessWar.exe', patterns: [] },
    notifications: {
      telegram_enabled: true,
      tray_enabled: true,
      cooldown_seconds: 60,
    },
  };
  let configLoaded = false;

  let isMonitoring = false;
  let isBotRunning = false;
  let newPatternName = '';
  let newPatternBytes = '';
  let newPatternDesc = '';
  let statusMessage = '';
  let statusTimeout: ReturnType<typeof setTimeout>;

  function showStatus(message: string) {
    statusMessage = message;
    clearTimeout(statusTimeout);
    statusTimeout = setTimeout(() => {
      statusMessage = '';
    }, 10000);
  }

  async function startMonitoring() {
    try {
      await invoke('start_monitoring');
      isMonitoring = true;
      showStatus('Мониторинг запущен');
    } catch (e) {
      showStatus(`Ошибка: ${e}`);
    }
  }

  async function stopMonitoring() {
    try {
      await invoke('stop_monitoring');
      isMonitoring = false;
      showStatus('Мониторинг остановлен');
    } catch (e) {
      showStatus(`Ошибка: ${e}`);
    }
  }

  async function addPattern() {
    if (!newPatternName || !newPatternBytes) return;
    try {
      await invoke('add_pattern', {
        name: newPatternName,
        bytes: newPatternBytes,
        description: newPatternDesc,
      });
      config = await invoke<Config>('get_config');
      newPatternName = '';
      newPatternBytes = '';
      newPatternDesc = '';
      showStatus('Паттерн добавлен');
    } catch (e) {
      showStatus(`Ошибка: ${e}`);
    }
  }

  async function removePattern(name: string) {
    try {
      await invoke('remove_pattern', { name });
      config = await invoke<Config>('get_config');
      showStatus('Паттерн удалён');
    } catch (e) {
      showStatus(`Ошибка: ${e}`);
    }
  }

  async function saveConfig() {
    try {
      await invoke('update_config', { newConfig: config });
      showStatus('Конфигурация сохранена');
    } catch (e) {
      showStatus(`Ошибка: ${e}`);
    }
  }

  async function testTelegram() {
    try {
      await invoke<string>('test_telegram', {
        token: config.telegram.bot_token,
      });
      showStatus('Telegram подключён!');
    } catch (e) {
      showStatus(`Ошибка: ${e}`);
    }
  }

  async function startBot() {
    if (!config.telegram.bot_token) {
      showStatus('Введите токен бота');
      return;
    }
    try {
      await invoke<string>('start_telegram_bot', {
        token: config.telegram.bot_token,
      });
      isBotRunning = true;
      showStatus('Бот запущен! Команда /start теперь доступна');
    } catch (e) {
      showStatus(`Ошибка: ${e}`);
    }
  }

  async function toggleUser(chatId: string, enabled: boolean) {
    try {
      await invoke<string>('set_user_enabled', { chatId, enabled });
      config = await invoke<Config>('get_config');
      showStatus(`Пользователь ${enabled ? 'включён' : 'отключён'}`);
    } catch (e) {
      showStatus(`Ошибка: ${e}`);
    }
  }

  async function toggleNotifyAll() {
    try {
      await invoke<string>('set_notify_all_users', { enabled: !config.telegram.notify_all_users });
      config = await invoke<Config>('get_config');
      showStatus(config.telegram.notify_all_users ? 'Рассылка включена' : 'Рассылка отключена');
    } catch (e) {
      showStatus(`Ошибка: ${e}`);
    }
  }

  onMount(async () => {
    try {
      config = await invoke<Config>('get_config');
      configLoaded = true;
      // Check if bot should be running (auto_start with token)
      if (config.telegram.bot_token && config.telegram.auto_start) {
        isBotRunning = true;
      }
    } catch (e) {
      console.error('Failed to load config:', e);
      configLoaded = true;
    }

    await listen('notification', (event: any) => {
      notifications = [
        { title: event.payload.title, body: event.payload.body, time: new Date().toLocaleTimeString() },
        ...notifications.slice(0, 49),
      ];
    });

    await listen('tray-start', async () => {
      try {
        await invoke('start_monitoring');
        isMonitoring = true;
        showStatus('Мониторинг запущен (из трея)');
      } catch (e) {
        showStatus(`Ошибка: ${e}`);
      }
    });

    await listen('tray-stop', async () => {
      try {
        await invoke('stop_monitoring');
        isMonitoring = false;
        showStatus('Мониторинг остановлен (из трея)');
      } catch (e) {
        showStatus(`Ошибка: ${e}`);
      }
    });

    // Refresh config periodically for user list updates
    setInterval(async () => {
      try {
        config = await invoke<Config>('get_config');
      } catch (e) {
        // Ignore errors
      }
    }, 5000);
  });

  let notifications: Array<{ title: string; body: string; time: string }> = [];

  function showPage(page: string) {
    currentPage = page;
  }
</script>

<div class="app-container">
  <aside class="sidebar">
    <div class="logo">
      <span class="logo-icon">⚔️</span>
      <span class="logo-text">EW Notify</span>
    </div>
    <nav class="nav">
      <button class="nav-item" class:active={currentPage === 'dashboard'} onclick={() => showPage('dashboard')}>
        <span class="nav-icon">📊</span>
        <span>Дашборд</span>
      </button>
      <button class="nav-item" class:active={currentPage === 'settings'} onclick={() => showPage('settings')}>
        <span class="nav-icon">⚙️</span>
        <span>Настройки</span>
      </button>
      <button class="nav-item" class:active={currentPage === 'patterns'} onclick={() => showPage('patterns')}>
        <span class="nav-icon">🔍</span>
        <span>Паттерны</span>
      </button>
      <button class="nav-item" class:active={currentPage === 'notifications'} onclick={() => showPage('notifications')}>
        <span class="nav-icon">🔔</span>
        <span>Уведомления</span>
      </button>
      <button class="nav-item" class:active={currentPage === 'users'} onclick={() => showPage('users')}>
        <span class="nav-icon">👥</span>
        <span>Пользователи</span>
      </button>
    </nav>
    <div class="sidebar-footer">
      <div class="version">v0.1.0</div>
    </div>
  </aside>

  <main class="content">
    {#if !configLoaded}
      <div class="loading">Загрузка...</div>
    {:else if currentPage === 'dashboard'}
      <div class="page-header">
        <h1>Дашборд</h1>
      </div>
      <div class="dashboard-grid">
        <div class="card status-card">
          <div class="card-header">
            <h2>Статус мониторинга</h2>
          </div>
          <div class="card-body">
            <div class="status-display">
              <div class="status-indicator" class:active={isMonitoring}></div>
              <span class="status-text">{isMonitoring ? 'Мониторинг активен' : 'Мониторинг остановлен'}</span>
            </div>
            <div class="status-controls">
              {#if isMonitoring}
                <button class="btn btn-danger" onclick={stopMonitoring}>⏹ Остановить</button>
              {:else}
                <button class="btn btn-success" onclick={startMonitoring}>▶ Начать мониторинг</button>
              {/if}
            </div>
          </div>
        </div>

        <div class="card info-card">
          <div class="card-header">
            <h2>Информация</h2>
          </div>
          <div class="card-body">
            <div class="info-row">
              <span class="info-label">Процесс:</span>
              <span class="info-value">{config.target?.process_name || 'EndlessWar.exe'}</span>
            </div>
            <div class="info-row">
              <span class="info-label">Паттернов:</span>
              <span class="info-value">{config.target?.patterns?.length || 0}</span>
            </div>
            <div class="info-row">
              <span class="info-label">Telegram-уведомления:</span>
              <span class="info-value" class:enabled={config.notifications?.telegram_enabled}>
                {config.notifications?.telegram_enabled ? 'Включены' : 'Выключены'}
              </span>
            </div>
            <div class="info-row">
              <span class="info-label">Telegram-бот:</span>
              <span class="info-value" class:enabled={isBotRunning}>
                {isBotRunning ? 'Запущен' : 'Остановлен'}
              </span>
            </div>
            <div class="info-row">
              <span class="info-label">Кулдаун:</span>
              <span class="info-value">{config.notifications?.cooldown_seconds || 60} сек</span>
            </div>
          </div>
        </div>

        <div class="card recent-card">
          <div class="card-header">
            <h2>Последние уведомления</h2>
          </div>
          <div class="card-body">
            {#if notifications.length === 0}
              <p class="empty-text">Нет уведомлений</p>
            {:else}
              <ul class="notification-list">
                {#each notifications.slice(0, 5) as notif}
                  <li>
                    <span class="notif-time">{notif.time}</span>
                    <span class="notif-title">{notif.title}</span>
                  </li>
                {/each}
              </ul>
            {/if}
          </div>
        </div>
      </div>
    {/if}

    {#if currentPage === 'settings'}
      <div class="page-header">
        <h1>Настройки</h1>
      </div>
      <div class="settings-grid">
        <div class="card">
          <div class="card-header">
            <h2>Telegram</h2>
          </div>
          <div class="card-body">
            <div class="form-group">
              <label>Bot Token</label>
              <input type="text" bind:value={config.telegram.bot_token} placeholder="123456789:ABCdefGHIjklMNOpqrsTUVwxyz" />
            </div>
            <div class="form-group">
              <label class="checkbox-label">
                <input type="checkbox" bind:checked={config.notifications.telegram_enabled} />
                <span>Включить Telegram уведомления</span>
              </label>
            </div>
            <div class="form-group">
              <label class="checkbox-label">
                <input type="checkbox" bind:checked={config.telegram.auto_start} />
                <span>Автозапуск бота при старте приложения</span>
              </label>
            </div>
            <div class="button-row">
              <button class="btn btn-secondary" onclick={testTelegram}>🔗 Проверить</button>
              <button class="btn btn-success" onclick={startBot} disabled={isBotRunning}>
                {isBotRunning ? '✅ Бот запущен' : '▶ Запустить бота'}
              </button>
            </div>
          </div>
        </div>

        <div class="card">
          <div class="card-header">
            <h2>Мониторинг</h2>
          </div>
          <div class="card-body">
            <div class="form-group">
              <label>Имя процесса</label>
              <input type="text" bind:value={config.target.process_name} placeholder="EndlessWar.exe" />
            </div>
            <div class="form-group">
              <label>Кулдаун (секунды)</label>
              <input type="number" bind:value={config.notifications.cooldown_seconds} min="0" />
            </div>
          </div>
        </div>

        <div class="card">
          <div class="card-header">
            <h2>Уведомления</h2>
          </div>
          <div class="card-body">
            <div class="form-group">
              <label class="checkbox-label">
                <input type="checkbox" bind:checked={config.notifications.tray_enabled} />
                <span>Системные уведомления</span>
              </label>
            </div>
          </div>
        </div>

        <div class="card-actions">
          <button class="btn btn-primary" onclick={saveConfig}>💾 Сохранить настройки</button>
        </div>
      </div>
    {/if}

    {#if currentPage === 'patterns'}
      <div class="page-header">
        <h1>Паттерны</h1>
      </div>
      <div class="patterns-page">
        <div class="card">
          <div class="card-header">
            <h2>Добавить паттерн</h2>
          </div>
          <div class="card-body">
            <div class="add-pattern-form">
              <div class="form-row">
                <div class="form-group">
                  <label>Название</label>
                  <input type="text" bind:value={newPatternName} placeholder="event_started" />
                </div>
                <div class="form-group">
                  <label>Hex байты</label>
                  <input type="text" bind:value={newPatternBytes} placeholder="A1B2C3D4" />
                </div>
              </div>
              <div class="form-group">
                <label>Описание</label>
                <input type="text" bind:value={newPatternDesc} placeholder="Событие началось" />
              </div>
              <button class="btn btn-success" onclick={addPattern}>➕ Добавить паттерн</button>
            </div>
          </div>
        </div>

        <div class="card">
          <div class="card-header">
            <h2>Список паттернов ({(config.target?.patterns || []).length})</h2>
          </div>
          <div class="card-body">
            {#if (config.target?.patterns || []).length === 0}
              <p class="empty-text">Нет паттернов</p>
            {:else}
              <ul class="pattern-list">
                {#each config.target.patterns as pattern}
                  <li class="pattern-item">
                    <div class="pattern-info">
                      <span class="pattern-name">{pattern.name}</span>
                      <span class="pattern-bytes">{pattern.bytes}</span>
                      <span class="pattern-desc">{pattern.description}</span>
                    </div>
                    <button class="btn btn-danger btn-sm" onclick={() => removePattern(pattern.name)}>🗑</button>
                  </li>
                {/each}
              </ul>
            {/if}
          </div>
        </div>
      </div>
    {/if}

    {#if currentPage === 'notifications'}
      <div class="page-header">
        <h1>История уведомлений</h1>
      </div>
      <div class="notifications-page">
        <div class="card">
          <div class="card-header">
            <h2>Все уведомления ({notifications.length})</h2>
          </div>
          <div class="card-body">
            {#if notifications.length === 0}
              <p class="empty-text">Нет уведомлений</p>
            {:else}
              <ul class="full-notification-list">
                {#each notifications as notif}
                  <li class="notification-item">
                    <div class="notif-content">
                      <span class="notif-time">{notif.time}</span>
                      <span class="notif-title">{notif.title}</span>
                      <span class="notif-body">{notif.body}</span>
                    </div>
                  </li>
                {/each}
              </ul>
            {/if}
          </div>
        </div>
      </div>
    {/if}

    {#if currentPage === 'users'}
      <div class="page-header">
        <h1>Управление пользователями</h1>
      </div>
      <div class="users-page">
        <div class="card">
          <div class="card-header">
            <h2>Зарегистрированные пользователи ({(config.telegram?.registered_users || []).length})</h2>
          </div>
          <div class="card-body">
            <div class="form-group">
              <label class="checkbox-label">
                <input 
                  type="checkbox" 
                  checked={config.telegram?.notify_all_users ?? true} 
                  onchange={() => toggleNotifyAll()} 
                />
                <span>Отправлять уведомления всем зарегистрированным пользователям</span>
              </label>
            </div>
            {#if (config.telegram?.registered_users || []).length === 0}
              <p class="empty-text">Нет зарегистрированных пользователей</p>
              <p class="hint-text">Пользователи автоматически добавляются при отправке команды /start боту</p>
            {:else}
              <ul class="user-list">
                {#each config.telegram.registered_users as user}
                  <li class="user-item">
                    <div class="user-info">
                      <span class="user-name">{user.first_name || 'Unknown'}</span>
                      <span class="user-username">@{user.username || '-'}</span>
                      <span class="user-id">ID: {user.chat_id}</span>
                    </div>
                    <div class="user-actions">
                      <button 
                        class="btn btn-sm" 
                        class:btn-success={user.enabled} 
                        class:btn-danger={!user.enabled}
                        onclick={() => toggleUser(user.chat_id, !user.enabled)}
                      >
                        {user.enabled ? '✅ Включён' : '❌ Отключён'}
                      </button>
                    </div>
                  </li>
                {/each}
              </ul>
            {/if}
          </div>
        </div>
      </div>
    {/if}

    {#if statusMessage}
      <div class="toast">{statusMessage}</div>
    {/if}
  </main>
</div>

<style>
  :global(*) {
    margin: 0;
    padding: 0;
    box-sizing: border-box;
  }

  :global(body) {
    font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif;
    background: #0f0f1a;
    color: #e0e0e0;
  }

  .app-container {
    display: flex;
    min-height: 100vh;
  }

  .sidebar {
    width: 240px;
    background: linear-gradient(180deg, #1a1a2e 0%, #16162a 100%);
    border-right: 1px solid #2a2a4a;
    display: flex;
    flex-direction: column;
    padding: 20px 0;
  }

  .logo {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 0 20px 30px;
    border-bottom: 1px solid #2a2a4a;
    margin-bottom: 20px;
  }

  .logo-icon {
    font-size: 28px;
  }

  .logo-text {
    font-size: 20px;
    font-weight: bold;
    color: #e94560;
  }

  .nav {
    flex: 1;
    display: flex;
    flex-direction: column;
    gap: 4px;
    padding: 0 12px;
  }

  .nav-item {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 14px 16px;
    border: none;
    background: transparent;
    color: #8888aa;
    border-radius: 10px;
    cursor: pointer;
    font-size: 15px;
    transition: all 0.2s ease;
    text-align: left;
  }

  .nav-item:hover {
    background: #252540;
    color: #aaaacc;
  }

  .nav-item.active {
    background: linear-gradient(135deg, #e94560 0%, #c73e54 100%);
    color: white;
    box-shadow: 0 4px 15px rgba(233, 69, 96, 0.3);
  }

  .nav-icon {
    font-size: 18px;
  }

  .sidebar-footer {
    padding: 20px;
    border-top: 1px solid #2a2a4a;
  }

  .version {
    color: #555577;
    font-size: 12px;
    text-align: center;
  }

  .content {
    flex: 1;
    padding: 30px 40px;
    overflow-y: auto;
  }

  .page-header {
    margin-bottom: 30px;
  }

  .page-header h1 {
    font-size: 28px;
    font-weight: 600;
    color: #ffffff;
  }

  .dashboard-grid {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(300px, 1fr));
    gap: 24px;
  }

  .settings-grid {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(350px, 1fr));
    gap: 24px;
  }

  .card {
    background: #1a1a2e;
    border-radius: 16px;
    border: 1px solid #2a2a4a;
    overflow: hidden;
  }

  .card-header {
    padding: 20px 24px;
    border-bottom: 1px solid #2a2a4a;
    background: #1e1e35;
  }

  .card-header h2 {
    font-size: 16px;
    font-weight: 600;
    color: #aaaacc;
  }

  .card-body {
    padding: 24px;
  }

  .status-display {
    display: flex;
    align-items: center;
    gap: 16px;
    margin-bottom: 24px;
  }

  .status-indicator {
    width: 20px;
    height: 20px;
    border-radius: 50%;
    background: #e94560;
    box-shadow: 0 0 20px rgba(233, 69, 96, 0.5);
  }

  .status-indicator.active {
    background: #4caf50;
    box-shadow: 0 0 20px rgba(76, 175, 80, 0.5);
    animation: pulse 1.5s infinite;
  }

  @keyframes pulse {
    0%, 100% { opacity: 1; }
    50% { opacity: 0.6; }
  }

  .status-text {
    font-size: 18px;
    font-weight: 500;
  }

  .status-controls {
    display: flex;
    gap: 12px;
  }

  .btn {
    padding: 14px 28px;
    border: none;
    border-radius: 10px;
    cursor: pointer;
    font-size: 15px;
    font-weight: 600;
    transition: all 0.2s ease;
  }

  .btn-success {
    background: linear-gradient(135deg, #4caf50 0%, #43a047 100%);
    color: white;
    box-shadow: 0 4px 15px rgba(76, 175, 80, 0.3);
  }

  .btn-success:hover {
    transform: translateY(-2px);
    box-shadow: 0 6px 20px rgba(76, 175, 80, 0.4);
  }

  .btn-danger {
    background: linear-gradient(135deg, #e94560 0%, #c73e54 100%);
    color: white;
    box-shadow: 0 4px 15px rgba(233, 69, 96, 0.3);
  }

  .btn-danger:hover {
    transform: translateY(-2px);
    box-shadow: 0 6px 20px rgba(233, 69, 96, 0.4);
  }

  .btn-secondary {
    background: linear-gradient(135deg, #6366f1 0%, #4f46e5 100%);
    color: white;
    box-shadow: 0 4px 15px rgba(99, 102, 241, 0.3);
  }

  .btn-secondary:hover {
    transform: translateY(-2px);
    box-shadow: 0 6px 20px rgba(99, 102, 241, 0.4);
  }

  .btn-primary {
    background: linear-gradient(135deg, #6366f1 0%, #4f46e5 100%);
    color: white;
    box-shadow: 0 4px 15px rgba(99, 102, 241, 0.3);
  }

  .btn-primary:hover {
    transform: translateY(-2px);
    box-shadow: 0 6px 20px rgba(99, 102, 241, 0.4);
  }

  .btn-sm {
    padding: 8px 12px;
    font-size: 13px;
  }

  .info-row {
    display: flex;
    justify-content: space-between;
    padding: 12px 0;
    border-bottom: 1px solid #2a2a4a;
  }

  .info-row:last-child {
    border-bottom: none;
  }

  .info-label {
    color: #8888aa;
  }

  .info-value {
    color: #ffffff;
    font-weight: 500;
  }

  .info-value.enabled {
    color: #4caf50;
  }

  .empty-text {
    color: #555577;
    text-align: center;
    padding: 40px;
  }

  .notification-list, .full-notification-list {
    list-style: none;
  }

  .notification-list li, .full-notification-list li {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 12px 0;
    border-bottom: 1px solid #2a2a4a;
  }

  .notification-list li:last-child, .full-notification-list li:last-child {
    border-bottom: none;
  }

  .notif-time {
    color: #555577;
    font-size: 13px;
    min-width: 70px;
  }

  .notif-title {
    font-weight: 500;
    color: #e94560;
  }

  .notif-content {
    display: flex;
    align-items: center;
    gap: 16px;
    width: 100%;
  }

  .notif-body {
    color: #8888aa;
    flex: 1;
  }

  .form-group {
    margin-bottom: 20px;
  }

  .form-group label {
    display: block;
    margin-bottom: 8px;
    color: #aaaacc;
    font-size: 14px;
    font-weight: 500;
  }

  .form-group input[type="text"],
  .form-group input[type="number"] {
    width: 100%;
    padding: 14px 16px;
    background: #16162a;
    border: 1px solid #2a2a4a;
    border-radius: 10px;
    color: #ffffff;
    font-size: 15px;
    transition: border-color 0.2s ease;
  }

  .form-group input:focus {
    outline: none;
    border-color: #e94560;
  }

  .form-row {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 16px;
  }

  .checkbox-label {
    display: flex;
    align-items: center;
    gap: 12px;
    cursor: pointer;
  }

  .checkbox-label input {
    width: 20px;
    height: 20px;
    accent-color: #e94560;
  }

  .card-actions {
    grid-column: 1 / -1;
    display: flex;
    justify-content: flex-end;
  }

  .add-pattern-form {
    display: flex;
    flex-direction: column;
    gap: 16px;
  }

  .add-pattern-form .btn {
    align-self: flex-start;
  }

  .pattern-list {
    list-style: none;
    display: flex;
    flex-direction: column;
    gap: 12px;
  }

  .pattern-item {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 16px;
    background: #16162a;
    border-radius: 10px;
    border: 1px solid #2a2a4a;
  }

  .pattern-info {
    display: flex;
    align-items: center;
    gap: 20px;
    flex: 1;
  }

  .pattern-name {
    font-weight: 600;
    color: #e94560;
    min-width: 120px;
  }

  .pattern-bytes {
    font-family: 'Fira Code', monospace;
    color: #4caf50;
    background: #1a2a1a;
    padding: 4px 10px;
    border-radius: 6px;
    font-size: 13px;
  }

  .pattern-desc {
    color: #8888aa;
    flex: 1;
  }

  .notifications-page .card-body {
    max-height: 600px;
    overflow-y: auto;
  }

  .toast {
    position: fixed;
    bottom: 30px;
    right: 30px;
    background: linear-gradient(135deg, #4caf50 0%, #43a047 100%);
    color: white;
    padding: 16px 28px;
    border-radius: 12px;
    font-weight: 500;
    box-shadow: 0 10px 30px rgba(0, 0, 0, 0.3);
    animation: slideIn 0.3s ease;
  }

  @keyframes slideIn {
    from {
      transform: translateY(20px);
      opacity: 0;
    }
    to {
      transform: translateY(0);
      opacity: 1;
    }
  }

  .button-row {
    display: flex;
    gap: 12px;
    margin-top: 16px;
  }

  .users-page .card-body {
    max-height: 600px;
    overflow-y: auto;
  }

  .user-list {
    list-style: none;
    display: flex;
    flex-direction: column;
    gap: 12px;
  }

  .user-item {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 16px;
    background: #16162a;
    border-radius: 10px;
    border: 1px solid #2a2a4a;
  }

  .user-info {
    display: flex;
    align-items: center;
    gap: 20px;
    flex: 1;
  }

  .user-name {
    font-weight: 600;
    color: #ffffff;
    min-width: 100px;
  }

  .user-username {
    color: #6366f1;
    min-width: 100px;
  }

  .user-id {
    color: #555577;
    font-size: 13px;
  }

  .user-actions {
    display: flex;
    gap: 8px;
  }

  .hint-text {
    color: #555577;
    font-size: 13px;
    text-align: center;
    padding: 20px;
  }

  .loading {
    display: flex;
    justify-content: center;
    align-items: center;
    height: 200px;
    color: #8888aa;
    font-size: 18px;
  }
</style>
