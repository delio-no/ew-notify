<script lang="ts">
  import { invoke } from '@tauri-apps/api/core';
  import { listen } from '@tauri-apps/api/event';
  import { onMount } from 'svelte';

  interface Pattern {
    name: string;
    bytes: string;
    description: string;
  }

  interface Config {
    telegram: {
      bot_token: string;
      chat_id: string;
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

  let config: Config = {
    telegram: { bot_token: '', chat_id: '' },
    target: { process_name: 'EndlessWar.exe', patterns: [] },
    notifications: {
      telegram_enabled: true,
      tray_enabled: true,
      cooldown_seconds: 60,
    },
  };

  let isMonitoring = false;
  let newPatternName = '';
  let newPatternBytes = '';
  let newPatternDesc = '';
  let statusMessage = '';
  let notifications: Array<{ title: string; body: string; time: string }> = [];

  onMount(async () => {
    try {
      config = await invoke<Config>('get_config');
    } catch (e) {
      console.error('Failed to load config:', e);
    }

    await listen('notification', (event: any) => {
      notifications = [
        { title: event.payload.title, body: event.payload.body, time: new Date().toLocaleTimeString() },
        ...notifications.slice(0, 9),
      ];
    });
  });

  async function startMonitoring() {
    try {
      await invoke('start_monitoring');
      isMonitoring = true;
      statusMessage = 'Мониторинг запущен';
    } catch (e) {
      statusMessage = `Ошибка: ${e}`;
    }
  }

  async function stopMonitoring() {
    try {
      await invoke('stop_monitoring');
      isMonitoring = false;
      statusMessage = 'Мониторинг остановлен';
    } catch (e) {
      statusMessage = `Ошибка: ${e}`;
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
      statusMessage = 'Паттерн добавлен';
    } catch (e) {
      statusMessage = `Ошибка: ${e}`;
    }
  }

  async function removePattern(name: string) {
    try {
      await invoke('remove_pattern', { name });
      config = await invoke<Config>('get_config');
      statusMessage = 'Паттерн удалён';
    } catch (e) {
      statusMessage = `Ошибка: ${e}`;
    }
  }

  async function saveConfig() {
    try {
      await invoke('update_config', { newConfig: config });
      statusMessage = 'Конфигурация сохранена';
    } catch (e) {
      statusMessage = `Ошибка: ${e}`;
    }
  }
</script>

<main>
  <h1>EndlessWar Notifier</h1>

  <section class="status">
    <div class="status-indicator" class:active={isMonitoring}></div>
    <span>{isMonitoring ? 'Мониторинг активен' : 'Мониторинг остановлен'}</span>
  </section>

  <section class="controls">
    {#if isMonitoring}
      <button class="stop" onclick={stopMonitoring}>Остановить мониторинг</button>
    {:else}
      <button class="start" onclick={startMonitoring}>Начать мониторинг</button>
    {/if}
  </section>

  <section class="config">
    <h2>Настройки</h2>
    <label>
      Имя процесса:
      <input type="text" bind:value={config.target.process_name} />
    </label>
    <label>
      Telegram Bot Token:
      <input type="text" bind:value={config.telegram.bot_token} />
    </label>
    <label>
      Telegram Chat ID:
      <input type="text" bind:value={config.telegram.chat_id} />
    </label>
    <label>
      <input type="checkbox" bind:checked={config.notifications.telegram_enabled} />
      Telegram уведомления
    </label>
    <label>
      <input type="checkbox" bind:checked={config.notifications.tray_enabled} />
      Системные уведомления
    </label>
    <label>
      Кулдаун (сек):
      <input type="number" bind:value={config.notifications.cooldown_seconds} />
    </label>
    <button onclick={saveConfig}>Сохранить настройки</button>
  </section>

  <section class="patterns">
    <h2>Паттерны</h2>
    <div class="add-pattern">
      <input type="text" placeholder="Название" bind:value={newPatternName} />
      <input type="text" placeholder="Hex (например: A1B2C3D4)" bind:value={newPatternBytes} />
      <input type="text" placeholder="Описание" bind:value={newPatternDesc} />
      <button onclick={addPattern}>Добавить</button>
    </div>
    <ul>
      {#each config.target.patterns as pattern}
        <li>
          <span class="pattern-name">{pattern.name}</span>
          <span class="pattern-bytes">{pattern.bytes}</span>
          <span class="pattern-desc">{pattern.description}</span>
          <button class="delete" onclick={() => removePattern(pattern.name)}>X</button>
        </li>
      {/each}
    </ul>
  </section>

  <section class="notifications">
    <h2>Уведомления</h2>
    <ul>
      {#each notifications as notif}
        <li>
          <span class="time">{notif.time}</span>
          <span class="title">{notif.title}</span>
          <span class="body">{notif.body}</span>
        </li>
      {/each}
    </ul>
  </section>

  {#if statusMessage}
    <div class="status-message">{statusMessage}</div>
  {/if}
</main>

<style>
  :global(body) {
    font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif;
    background: #1a1a2e;
    color: #eee;
    margin: 0;
    padding: 20px;
  }

  main {
    max-width: 800px;
    margin: 0 auto;
  }

  h1 {
    color: #e94560;
    margin-bottom: 20px;
  }

  h2 {
    color: #0f3460;
    background: #16213e;
    padding: 10px;
    border-radius: 5px;
  }

  section {
    background: #16213e;
    padding: 20px;
    margin-bottom: 20px;
    border-radius: 10px;
  }

  .status {
    display: flex;
    align-items: center;
    gap: 10px;
  }

  .status-indicator {
    width: 15px;
    height: 15px;
    border-radius: 50%;
    background: #e94560;
  }

  .status-indicator.active {
    background: #4caf50;
    animation: pulse 1s infinite;
  }

  @keyframes pulse {
    0%, 100% { opacity: 1; }
    50% { opacity: 0.5; }
  }

  button {
    padding: 10px 20px;
    border: none;
    border-radius: 5px;
    cursor: pointer;
    font-weight: bold;
    margin: 5px;
  }

  .start {
    background: #4caf50;
    color: white;
  }

  .stop {
    background: #e94560;
    color: white;
  }

  .delete {
    background: #e94560;
    color: white;
    padding: 5px 10px;
  }

  label {
    display: block;
    margin: 10px 0;
  }

  input[type="text"],
  input[type="number"] {
    padding: 8px;
    border-radius: 5px;
    border: 1px solid #0f3460;
    background: #1a1a2e;
    color: #eee;
    width: 100%;
    box-sizing: border-box;
  }

  .add-pattern {
    display: flex;
    gap: 10px;
    flex-wrap: wrap;
  }

  .add-pattern input {
    flex: 1;
    min-width: 150px;
  }

  ul {
    list-style: none;
    padding: 0;
  }

  li {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 10px;
    background: #1a1a2e;
    margin: 5px 0;
    border-radius: 5px;
  }

  .pattern-name {
    font-weight: bold;
    color: #e94560;
  }

  .pattern-bytes {
    font-family: monospace;
    color: #4caf50;
  }

  .pattern-desc {
    flex: 1;
    color: #aaa;
  }

  .status-message {
    position: fixed;
    bottom: 20px;
    right: 20px;
    background: #4caf50;
    color: white;
    padding: 15px 20px;
    border-radius: 5px;
  }
</style>