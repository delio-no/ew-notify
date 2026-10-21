<script lang="ts">
  import { listen } from '@tauri-apps/api/event';
  import { onMount } from 'svelte';
  import { api } from './lib/api';
  import {
    defaultConfig,
    type Config,
    type MonitoringStatus,
    type PageId,
    type PacketLogEntry,
    type Pattern,
    type StoredNotification,
  } from './lib/types';
  import Sidebar from './components/Sidebar.svelte';
  import Toast from './components/Toast.svelte';
  import DashboardPage from './components/pages/DashboardPage.svelte';
  import SettingsPage from './components/pages/SettingsPage.svelte';
  import PatternsPage from './components/pages/PatternsPage.svelte';
  import NotificationsPage from './components/pages/NotificationsPage.svelte';
  import UsersPage from './components/pages/UsersPage.svelte';
  import ConsolePage from './components/pages/ConsolePage.svelte';

  let currentPage = $state<PageId>('dashboard');
  let config = $state<Config>(defaultConfig());
  let configLoaded = $state(false);
  let isMonitoring = $state(false);
  let isBotRunning = $state(false);
  let monitorStatus = $state<MonitoringStatus>({
    active: false,
    process_name: '',
    tcp_port: null,
    filter: '',
    packets_seen: 0,
    matches_seen: 0,
    dedup_suppressed: 0,
    process_filter_active: false,
    bot_running: false,
    last_error: '',
    is_admin: false,
    config_path: '',
    log_path: '',
    capture_mode: 'auto',
    discovered_endpoints: [],
    packet_log_enabled: false,
    packet_log_skipped: 0,
  });
  let packetsSeen = $state(0);
  let matchesSeen = $state(0);
  let dedupSuppressed = $state(0);
  let isAdmin = $state(false);
  let configPath = $state('');
  let logPath = $state('');
  let captureMode = $state('auto');
  let discoveredEndpoints = $state<string[]>([]);
  let activeFilter = $state('');
  let activePort = $state<number | null>(null);
  let packetLogEnabled = $state(false);
  let packetLines = $state<PacketLogEntry[]>([]);
  let showBotToken = $state(false);
  let lastError = $state('');
  let lastPollAt = $state('—');
  let pollErrors = $state(0);
  let logLines = $state<Array<{ time: string; message: string }>>([]);
  let consoleAutoScroll = $state(true);
  let consoleEl = $state<HTMLPreElement | undefined>(undefined);
  let newPatternName = $state('');
  let newPatternBytes = $state('');
  let newPatternDesc = $state('');
  let patternTouched = $state({ name: false, bytes: false, desc: false });
  let patternSubmitAttempted = $state(false);
  let testHexInput = $state('');
  let testHexResult = $state('');
  let statusMessage = $state('');
  let statusTimeout: ReturnType<typeof setTimeout>;
  let notifications = $state<StoredNotification[]>([]);

  function patternNameError(value: string): string | null {
    const name = value.trim();
    if (!name) return 'Укажи название паттерна';
    if (!/^[A-Za-z][A-Za-z0-9_]*$/.test(name)) {
      return 'Латиница, цифры и _ без пробелов (например STOP_FARM)';
    }
    return null;
  }

  function patternBytesError(value: string): string | null {
    const raw = value.trim();
    if (!raw) return 'Укажи hex-байты';
    const cleaned = raw.replace(/\s+/g, '');
    if (!/^[0-9A-Fa-f]+$/.test(cleaned)) {
      return 'Только шестнадцатеричные символы 0-9 A-F (пробелы можно)';
    }
    if (cleaned.length % 2 !== 0) return 'Нечётная длина — байты идут парами';
    if (cleaned.length < 2) return 'Минимум 1 байт';
    return null;
  }

  function patternDescError(value: string): string | null {
    if (!value.trim()) return 'Описание события обязательно';
    return null;
  }

  let nameFieldError = $derived(
    patternTouched.name || patternSubmitAttempted ? patternNameError(newPatternName) : null,
  );
  let bytesFieldError = $derived(
    patternTouched.bytes || patternSubmitAttempted ? patternBytesError(newPatternBytes) : null,
  );
  let descFieldError = $derived(
    patternTouched.desc || patternSubmitAttempted ? patternDescError(newPatternDesc) : null,
  );
  let isCaptureAuto = $derived((config.target.capture_mode || 'auto') === 'auto');

  function showStatus(message: string) {
    statusMessage = message;
    clearTimeout(statusTimeout);
    statusTimeout = setTimeout(() => {
      statusMessage = '';
    }, 10000);
  }

  function applyMonitorStatus(status: MonitoringStatus) {
    monitorStatus = { ...status };
    isMonitoring = !!status.active;
    isBotRunning = !!status.bot_running;
    packetsSeen = Number(status.packets_seen) || 0;
    matchesSeen = Number(status.matches_seen) || 0;
    dedupSuppressed = Number(status.dedup_suppressed) || 0;
    isAdmin = !!status.is_admin;
    configPath = status.config_path || '';
    logPath = status.log_path || '';
    captureMode = status.capture_mode || config.target.capture_mode || 'auto';
    discoveredEndpoints = status.discovered_endpoints || [];
    activeFilter = status.filter || '';
    activePort = status.tcp_port;
    packetLogEnabled = !!status.packet_log_enabled;
    lastError = status.last_error || '';
    lastPollAt = new Date().toLocaleTimeString();
  }

  async function refreshUsers() {
    try {
      const users = await api.getRegisteredUsers();
      const cfg = await api.getConfig();
      config = {
        ...config,
        telegram: {
          ...config.telegram,
          registered_users: [...users],
          notify_all_users: cfg.telegram.notify_all_users,
          bot_token: cfg.telegram.bot_token || config.telegram.bot_token,
          admin_chat_id: cfg.telegram.admin_chat_id ?? config.telegram.admin_chat_id ?? '',
          auto_start: cfg.telegram.auto_start,
        },
      };
    } catch (e) {
      console.error('refreshUsers failed', e);
    }
  }

  async function refreshMonitorStatus() {
    try {
      const status = await api.getMonitoringStatus();
      applyMonitorStatus(status);
      const notes = await api.getRecentNotifications();
      notifications = notes.map((n) => ({ title: n.title, body: n.body, time: n.time }));
    } catch (e) {
      pollErrors += 1;
      console.error('refreshMonitorStatus failed', e);
      lastError = String(e);
    }
  }

  async function refreshLogs() {
    try {
      logLines = await api.getRecentLogs();
      if (consoleAutoScroll && consoleEl) {
        requestAnimationFrame(() => {
          if (consoleEl) consoleEl.scrollTop = consoleEl.scrollHeight;
        });
      }
    } catch (e) {
      console.error('refreshLogs failed', e);
    }
  }

  async function refreshPackets() {
    try {
      packetLines = await api.getRecentPackets();
    } catch (e) {
      console.error('refreshPackets failed', e);
    }
  }

  async function clearLogs() {
    try {
      await api.clearLogs();
      logLines = [];
      showStatus('Консоль очищена');
    } catch (e) {
      showStatus(`Ошибка: ${e}`);
    }
  }

  async function clearPacketLog() {
    try {
      await api.clearPacketLog();
      packetLines = [];
      showStatus('Лог пакетов очищен');
    } catch (e) {
      showStatus(`Ошибка: ${e}`);
    }
  }

  async function togglePacketLog() {
    try {
      const next = !packetLogEnabled;
      await api.setPacketLogEnabled(next);
      packetLogEnabled = next;
      if (!next) packetLines = [];
      showStatus(next ? 'Лог пакетов включён' : 'Лог пакетов выключен');
    } catch (e) {
      showStatus(`Ошибка: ${e}`);
    }
  }

  async function createPatternFromHex(hex: string) {
    const cleaned = hex.replace(/\s+/g, '');
    if (!cleaned) {
      showStatus('Пустой hex');
      return;
    }
    const name = prompt('ID паттерна (латиница), например CUSTOM_EVENT:');
    if (name == null) return;
    const description = prompt('Описание события:');
    if (description == null) return;
    try {
      await api.addPattern(name.trim(), cleaned, description.trim());
      config = await api.getConfig();
      showStatus(`Паттерн ${name.trim()} добавлен`);
      currentPage = 'patterns';
    } catch (e) {
      showStatus(`Ошибка: ${e}`);
    }
  }

  async function startMonitoring() {
    try {
      const result = await api.startMonitoring();
      isMonitoring = true;
      await refreshMonitorStatus();
      showStatus(result || 'Мониторинг запущен');
    } catch (e) {
      showStatus(`Ошибка: ${e}`);
      lastError = String(e);
    }
  }

  async function stopMonitoring() {
    try {
      await api.stopMonitoring();
      isMonitoring = false;
      await refreshMonitorStatus();
      showStatus('Мониторинг остановлен');
    } catch (e) {
      showStatus(`Ошибка: ${e}`);
    }
  }

  async function hideToTray() {
    try {
      await api.hideToTray();
    } catch (e) {
      showStatus(`Ошибка: ${e}`);
    }
  }

  async function addPattern() {
    patternSubmitAttempted = true;
    patternTouched = { name: true, bytes: true, desc: true };
    if (
      patternNameError(newPatternName) ||
      patternBytesError(newPatternBytes) ||
      patternDescError(newPatternDesc)
    ) {
      return;
    }
    try {
      await api.addPattern(
        newPatternName.trim(),
        newPatternBytes.trim().replace(/\s+/g, ''),
        newPatternDesc.trim(),
      );
      config = await api.getConfig();
      newPatternName = '';
      newPatternBytes = '';
      newPatternDesc = '';
      patternTouched = { name: false, bytes: false, desc: false };
      patternSubmitAttempted = false;
      showStatus('Паттерн добавлен');
    } catch (e) {
      showStatus(`Ошибка: ${e}`);
    }
  }

  async function removePattern(name: string) {
    try {
      await api.removePattern(name);
      config = await api.getConfig();
      showStatus('Паттерн удалён');
    } catch (e) {
      showStatus(`Ошибка: ${e}`);
    }
  }

  async function resetPatterns() {
    if (!confirm('Сбросить паттерны к DEATH / STOP_FARM?')) return;
    try {
      const patterns = await api.resetPatterns();
      config = { ...config, target: { ...config.target, patterns } };
      showStatus('Паттерны сброшены');
    } catch (e) {
      showStatus(`Ошибка: ${e}`);
    }
  }

  async function exportPatterns() {
    try {
      await navigator.clipboard.writeText(await api.exportPatterns());
      showStatus('Паттерны скопированы в буфер');
    } catch (e) {
      showStatus(`Ошибка экспорта: ${e}`);
    }
  }

  async function importPatterns(replace: boolean) {
    const raw = prompt(
      replace ? 'JSON паттернов (замена):' : 'JSON паттернов (merge):',
    );
    if (raw == null || !raw.trim()) return;
    try {
      const patterns = await api.importPatterns(raw.trim(), replace);
      config = { ...config, target: { ...config.target, patterns } };
      showStatus(`Импорт: паттернов ${patterns.length}`);
    } catch (e) {
      showStatus(`Ошибка импорта: ${e}`);
    }
  }

  async function runHexTest() {
    const hex = testHexInput.trim();
    if (!hex) {
      testHexResult = 'Вставь hex payload';
      return;
    }
    try {
      const r = await api.testPatternHex(hex);
      testHexResult = r.matched ? `MATCH: ${r.name} — ${r.description}` : 'NO MATCH';
    } catch (e) {
      testHexResult = `Ошибка: ${e}`;
    }
  }

  async function resetConfig() {
    if (
      !confirm(
        'Сбросить все настройки?\nПаттерны DEATH/STOP_FARM, порт 7777. Токен и пользователи очистятся.',
      )
    ) {
      return;
    }
    try {
      const wasMonitoring = isMonitoring;
      const defaults = await api.resetConfig();
      config = {
        ...defaults,
        telegram: {
          ...defaults.telegram,
          admin_chat_id: defaults.telegram.admin_chat_id ?? '',
        },
        startup: {
          auto_start_monitoring: !!defaults.startup?.auto_start_monitoring,
          start_minimized_to_tray: !!defaults.startup?.start_minimized_to_tray,
          close_to_tray: defaults.startup?.close_to_tray !== false,
        },
      };
      if (wasMonitoring) {
        await startMonitoring();
        showStatus('Настройки сброшены, мониторинг перезапущен');
      } else {
        await refreshMonitorStatus();
        showStatus('Настройки сброшены');
      }
    } catch (e) {
      showStatus(`Ошибка: ${e}`);
    }
  }

  async function saveConfig() {
    try {
      const wasMonitoring = isMonitoring;
      const prevPort = monitorStatus.tcp_port;
      const tcpPort = config.target.tcp_port;
      const latest = await api.getConfig();
      const patterns =
        (config.target.patterns?.length ? config.target.patterns : null) ||
        latest.target?.patterns ||
        [];
      const remoteRaw = (config.target.remote_ip ?? '').toString().trim();
      const payload: Config = {
        ...config,
        telegram: {
          ...config.telegram,
          admin_chat_id: config.telegram.admin_chat_id ?? '',
          registered_users: config.telegram.registered_users?.length
            ? config.telegram.registered_users
            : latest.telegram?.registered_users || [],
        },
        target: {
          ...config.target,
          capture_mode: config.target.capture_mode === 'manual' ? 'manual' : 'auto',
          tcp_port:
            tcpPort === null || tcpPort === undefined || (tcpPort as unknown) === ''
              ? 7777
              : Number(tcpPort),
          udp_port: config.target.udp_port ?? null,
          remote_ip: remoteRaw || null,
          patterns,
        },
        notifications: {
          ...config.notifications,
          cooldown_seconds: Number(config.notifications.cooldown_seconds) || 0,
          burst_dedup_ms:
            config.notifications.burst_dedup_ms === undefined ||
            config.notifications.burst_dedup_ms === null ||
            (config.notifications.burst_dedup_ms as unknown) === ''
              ? 2000
              : Number(config.notifications.burst_dedup_ms),
        },
        startup: {
          auto_start_monitoring: !!config.startup?.auto_start_monitoring,
          start_minimized_to_tray: !!config.startup?.start_minimized_to_tray,
          close_to_tray: config.startup?.close_to_tray !== false,
        },
      };
      const result = await api.updateConfig(payload);
      config = payload;
      await refreshMonitorStatus();
      if (result.includes('monitoring restarted') || result.includes('restart failed')) {
        showStatus(result);
      } else if (wasMonitoring && prevPort !== payload.target.tcp_port) {
        await startMonitoring();
        showStatus('Конфиг сохранён, мониторинг перезапущен');
      } else {
        showStatus(result.includes('Config updated') ? 'Конфигурация сохранена' : result);
      }
    } catch (e) {
      showStatus(`Ошибка: ${e}`);
    }
  }

  async function testTelegram() {
    try {
      await api.testTelegram(config.telegram.bot_token);
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
      await api.startTelegramBot(config.telegram.bot_token);
      isBotRunning = true;
      await refreshMonitorStatus();
      showStatus('Бот запущен');
    } catch (e) {
      showStatus(`Ошибка: ${e}`);
    }
  }

  async function restartBot() {
    if (!config.telegram.bot_token) {
      showStatus('Введите токен бота');
      return;
    }
    try {
      await api.stopTelegramBot();
      await new Promise((r) => setTimeout(r, 700));
      await api.startTelegramBot(config.telegram.bot_token);
      isBotRunning = true;
      await refreshMonitorStatus();
      showStatus('Бот перезапущен');
    } catch (e) {
      showStatus(`Ошибка: ${e}`);
    }
  }

  async function stopBot() {
    try {
      await api.stopTelegramBot();
      isBotRunning = false;
      await refreshMonitorStatus();
      showStatus('Бот остановлен');
    } catch (e) {
      showStatus(`Ошибка: ${e}`);
    }
  }

  async function toggleUser(chatId: string, enabled: boolean) {
    try {
      await api.setUserEnabled(chatId, enabled);
      await refreshUsers();
      showStatus(`Пользователь ${enabled ? 'включён' : 'отключён'}`);
    } catch (e) {
      showStatus(`Ошибка: ${e}`);
    }
  }

  async function toggleNotifyAll() {
    try {
      await api.setNotifyAllUsers(!config.telegram.notify_all_users);
      await refreshUsers();
      showStatus(config.telegram.notify_all_users ? 'Рассылка включена' : 'Рассылка отключена');
    } catch (e) {
      showStatus(`Ошибка: ${e}`);
    }
  }

  function showPage(page: PageId) {
    currentPage = page;
    if (page === 'users') void refreshUsers();
    if (page === 'dashboard') void refreshMonitorStatus();
    if (page === 'console') void refreshLogs();
  }

  onMount(() => {
    const statusTimer = setInterval(() => {
      void refreshMonitorStatus();
      if (currentPage === 'console') {
        void refreshLogs();
        if (packetLogEnabled) void refreshPackets();
      }
    }, 500);
    const usersTimer = setInterval(() => {
      void refreshUsers();
    }, 2000);

    const unlisteners: Array<() => void> = [];

    void (async () => {
      try {
        const loaded = await api.getConfig();
        config = {
          ...loaded,
          telegram: {
            ...loaded.telegram,
            admin_chat_id: loaded.telegram.admin_chat_id ?? '',
          },
          target: {
            ...loaded.target,
            capture_mode: loaded.target?.capture_mode === 'manual' ? 'manual' : 'auto',
            remote_ip: loaded.target?.remote_ip ?? null,
            patterns: loaded.target?.patterns || [],
          },
          notifications: {
            ...loaded.notifications,
            burst_dedup_ms: loaded.notifications?.burst_dedup_ms ?? 2000,
          },
          startup: {
            auto_start_monitoring: !!loaded.startup?.auto_start_monitoring,
            start_minimized_to_tray: !!loaded.startup?.start_minimized_to_tray,
            close_to_tray: loaded.startup?.close_to_tray !== false,
          },
        };
        await refreshMonitorStatus();
      } catch (e) {
        console.error('Failed to load config:', e);
        lastError = String(e);
      } finally {
        configLoaded = true;
      }

      try {
        unlisteners.push(
          await listen<{ title: string; body: string }>('notification', (event) => {
            showStatus(`${event.payload.title}: ${event.payload.body}`);
            void refreshMonitorStatus();
          }),
        );
        unlisteners.push(await listen('users-updated', () => void refreshUsers()));
        unlisteners.push(
          await listen<string>('bot-error', (event) => {
            lastError = event.payload || 'Telegram error';
            showStatus(lastError);
          }),
        );
        unlisteners.push(
          await listen('tray-start', async () => {
            await refreshMonitorStatus();
            showStatus('Мониторинг запущен (из трея)');
          }),
        );
        unlisteners.push(
          await listen('tray-stop', async () => {
            await refreshMonitorStatus();
            showStatus('Мониторинг остановлен (из трея)');
          }),
        );
      } catch (e) {
        console.error('listen setup failed', e);
      }
    })();

    return () => {
      clearInterval(statusTimer);
      clearInterval(usersTimer);
      for (const off of unlisteners) off();
    };
  });
</script>

<div class="app-container">
  <Sidebar {currentPage} {isAdmin} onNavigate={showPage} />

  <main class="content">
    {#if !configLoaded}
      <div class="loading">Загрузка...</div>
    {:else if currentPage === 'dashboard'}
      <DashboardPage
        {config}
        {isMonitoring}
        {isBotRunning}
        {isAdmin}
        {captureMode}
        {discoveredEndpoints}
        {activePort}
        {isCaptureAuto}
        {packetsSeen}
        {matchesSeen}
        {dedupSuppressed}
        {lastError}
        {activeFilter}
        {configPath}
        {logPath}
        {lastPollAt}
        {pollErrors}
        monitorProcessName={monitorStatus.process_name}
        {notifications}
        onStart={startMonitoring}
        onStop={stopMonitoring}
        onHideTray={hideToTray}
      />
    {:else if currentPage === 'settings'}
      <SettingsPage
        bind:config
        {isBotRunning}
        {isCaptureAuto}
        bind:showBotToken
        onSave={saveConfig}
        onReset={resetConfig}
        onTestTelegram={testTelegram}
        onStartBot={startBot}
        onStopBot={stopBot}
        onRestartBot={restartBot}
      />
    {:else if currentPage === 'patterns'}
      <PatternsPage
        {config}
        bind:newPatternName
        bind:newPatternBytes
        bind:newPatternDesc
        bind:testHexInput
        {testHexResult}
        {nameFieldError}
        {bytesFieldError}
        {descFieldError}
        onTouchName={() => (patternTouched.name = true)}
        onTouchBytes={() => (patternTouched.bytes = true)}
        onTouchDesc={() => (patternTouched.desc = true)}
        onAdd={addPattern}
        onRemove={removePattern}
        onReset={resetPatterns}
        onExport={exportPatterns}
        onImport={importPatterns}
        onTestHex={runHexTest}
      />
    {:else if currentPage === 'notifications'}
      <NotificationsPage {notifications} />
    {:else if currentPage === 'users'}
      <UsersPage
        {config}
        onRefresh={refreshUsers}
        onToggleUser={toggleUser}
        onToggleNotifyAll={toggleNotifyAll}
      />
    {:else if currentPage === 'console'}
      <ConsolePage
        {logLines}
        {packetLines}
        {packetLogEnabled}
        bind:consoleAutoScroll
        bind:consoleEl
        onRefreshLogs={refreshLogs}
        onClearLogs={clearLogs}
        onTogglePacketLog={togglePacketLog}
        onRefreshPackets={refreshPackets}
        onClearPackets={clearPacketLog}
        onCreatePattern={createPatternFromHex}
      />
    {/if}

    <Toast message={statusMessage} />
  </main>
</div>
