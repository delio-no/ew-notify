<script lang="ts">
  import { listen } from '@tauri-apps/api/event';
  import { onMount } from 'svelte';
  import { api } from './lib/api';
  import {
    defaultConfig,
    type ClickerConfig,
    type ClickerStatus,
    type ClickerTarget,
    type Config,
    type MonitoringStatus,
    type PageId,
    type PacketLogEntry,
    type Pattern,
    type StoredNotification,
    type TargetWindow,
    type UpdateInfo,
    type WindowFilterMode,
  } from './lib/types';
  import Sidebar from './components/Sidebar.svelte';
  import Toast from './components/Toast.svelte';
  import DashboardPage from './components/pages/DashboardPage.svelte';
  import SettingsPage from './components/pages/SettingsPage.svelte';
  import PatternsPage from './components/pages/PatternsPage.svelte';
  import NotificationsPage from './components/pages/NotificationsPage.svelte';
  import UsersPage from './components/pages/UsersPage.svelte';
  import ConsolePage from './components/pages/ConsolePage.svelte';
  import ClickerPage from './components/pages/ClickerPage.svelte';
  import StreamPage from './components/pages/StreamPage.svelte';

  let currentPage = $state<PageId>('dashboard');
  let config = $state<Config>(defaultConfig());
  let configLoaded = $state(false);
  let isMonitoring = $state(false);
  let isBotRunning = $state(false);
  let savedToken = $state('');
  let isTokenChanged = $derived(config.telegram.bot_token.trim() !== savedToken.trim());
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
  let availableUpdate = $state<UpdateInfo | null>(null);
  let isUpdating = $state(false);
  let updateError = $state<string | null>(null);
  let isStreamLive = $state(false);

  async function handleDirectUpdate() {
    if (!availableUpdate || !availableUpdate.download_url || isUpdating) return;
    isUpdating = true;
    updateError = null;
    showStatus(`Скачивание обновления ${availableUpdate.latest_version}...`);
    try {
      await api.installUpdate(availableUpdate.download_url, availableUpdate.asset_name);
    } catch (e) {
      isUpdating = false;
      const msg = `Ошибка установки обновления: ${e}`;
      updateError = msg;
      showStatus(msg);
    }
  }
  let lastError = $state('');
  let lastPollAt = $state('—');
  let pollErrors = $state(0);
  let logLines = $state<Array<{ time: string; message: string }>>([]);
  let consoleAutoScroll = $state(true);
  let consoleEl = $state<HTMLPreElement | undefined>(undefined);
  let newPatternName = $state('');
  let newPatternBytes = $state('');
  let newPatternDesc = $state('');
  let newPatternNotifyEnabled = $state(true);
  let newPatternScreenshotEnabled = $state(false);
  let patternTouched = $state({ name: false, bytes: false, desc: false });
  let patternSubmitAttempted = $state(false);
  let testHexInput = $state('');
  let testHexResult = $state('');
  let statusMessage = $state('');
  let statusTimeout: ReturnType<typeof setTimeout>;
  let notifications = $state<StoredNotification[]>([]);
  let windowFilterMode = $state<WindowFilterMode>({ mode: 'All' });
  let targetWindows = $state<TargetWindow[]>([]);
  let clickerStatus = $state<ClickerStatus>({
    running: false,
    count: 0,
    last_stop_reason: null,
    config: {
      rules: [
        {
          id: 'rule-1',
          name: 'Клавиша 1',
          enabled: true,
          key: '1',
          interval_ms: 1000,
          target: { type: 'Captured' },
          stop_on_pattern: false,
          stop_patterns: [],
          start_on_pattern: false,
          start_patterns: [],
        },
      ],
      notify_on_stop: true,
      notify_on_start: true,
    },
  });

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
      config = {
        ...config,
        telegram: {
          ...config.telegram,
          registered_users: [...users],
        },
      };
    } catch (e) {
      console.error('refreshUsers failed', e);
    }
  }

  async function refreshWindowFilter() {
    try {
      windowFilterMode = await api.getWindowFilterStatus();
      targetWindows = await api.getTargetWindows();
    } catch (e) {
      console.error('refreshWindowFilter failed', e);
    }
  }

  async function selectWindowFilter(slot: number, pid: number) {
    try {
      windowFilterMode = await api.setWindowFilter(slot, pid);
      showStatus(`Выбрано окно #${slot} [PID ${pid}]`);
    } catch (e) {
      showStatus(`Ошибка выбора окна: ${e}`);
    }
  }

  async function removeWindowSlot(slot: number) {
    try {
      windowFilterMode = await api.removeWindowSlot(slot);
      showStatus(`Слот #${slot} снят с отслеживания`);
    } catch (e) {
      showStatus(`Ошибка снятия слота: ${e}`);
    }
  }

  async function resetWindowFilter() {
    try {
      windowFilterMode = await api.resetWindowFilter();
      showStatus('Отслеживаются все окна');
    } catch (e) {
      showStatus(`Ошибка сброса: ${e}`);
    }
  }

  async function refreshMonitorStatus() {
    try {
      const status = await api.getMonitoringStatus();
      applyMonitorStatus(status);
      const notes = await api.getRecentNotifications();
      notifications = notes.map((n) => ({ title: n.title, body: n.body, time: n.time }));
      void refreshWindowFilter();
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
        newPatternNotifyEnabled,
        newPatternScreenshotEnabled
      );
      config = await api.getConfig();
      newPatternName = '';
      newPatternBytes = '';
      newPatternDesc = '';
      newPatternNotifyEnabled = true;
      newPatternScreenshotEnabled = false;
      patternTouched = { name: false, bytes: false, desc: false };
      patternSubmitAttempted = false;
      showStatus('Паттерн добавлен');
    } catch (e) {
      showStatus(`Ошибка: ${e}`);
    }
  }

  async function togglePatternNotify(pattern: Pattern) {
    const next = !(pattern.notify_enabled ?? true);
    pattern.notify_enabled = next;
    try {
      await api.setPatternNotifyEnabled(pattern.name, next);
      showStatus(
        next
          ? `Уведомления для ${pattern.name} включены`
          : `Уведомления для ${pattern.name} выключены`
      );
    } catch {
      try {
        await api.updateConfig(config);
        showStatus(
          next
            ? `Уведомления для ${pattern.name} включены`
            : `Уведомления для ${pattern.name} выключены`
        );
      } catch (err) {
        showStatus(`Ошибка: ${err}`);
      }
    }
  }

  async function togglePatternScreenshot(pattern: Pattern) {
    const next = !(pattern.screenshot_enabled ?? true);
    pattern.screenshot_enabled = next;
    try {
      await api.setPatternScreenshotEnabled(pattern.name, next);
      showStatus(
        next
          ? `Скриншоты для ${pattern.name} включены`
          : `Скриншоты для ${pattern.name} выключены`
      );
    } catch {
      try {
        await api.updateConfig(config);
        showStatus(
          next
            ? `Скриншоты для ${pattern.name} включены`
            : `Скриншоты для ${pattern.name} выключены`
        );
      } catch (err) {
        showStatus(`Ошибка: ${err}`);
      }
    }
  }

  async function testSendKey(key: string, target?: ClickerTarget) {
    try {
      const res = await api.testSendKey(key, target);
      showStatus(res);
    } catch (e) {
      showStatus(`Ошибка теста: ${e}`);
    }
  }

  async function testSystemNotification() {
    try {
      const res = await api.testSystemNotification();
      showStatus(res);
    } catch (e) {
      showStatus(`Ошибка теста уведомлений: ${e}`);
    }
  }



  async function refreshClickerStatus() {
    try {
      clickerStatus = await api.getClickerStatus();
    } catch (e) {
      console.error('Failed to get clicker status:', e);
    }
  }

  async function startClicker() {
    try {
      await api.startClicker();
      showStatus('Автокликер запущен');
      await refreshClickerStatus();
    } catch (e) {
      showStatus(`Ошибка запуска кликера: ${e}`);
    }
  }

  async function stopClicker() {
    try {
      await api.stopClicker();
      showStatus('Автокликер остановлен');
      await refreshClickerStatus();
    } catch (e) {
      showStatus(`Ошибка остановки кликера: ${e}`);
    }
  }

  async function saveClickerConfig(cfg: ClickerConfig) {
    try {
      await api.saveClickerConfig(cfg);
      config.clicker = { ...cfg };
      clickerStatus.config = { ...cfg };
      showStatus('Настройки кликера сохранены');
    } catch (e) {
      showStatus(`Ошибка сохранения настроек кликера: ${e}`);
    }
  }

  async function removePattern(name: string) {
    if (name === 'DEATH' || name === 'STOP_FARM') {
      showStatus('Базовые паттерны DEATH и STOP_FARM нельзя удалить');
      return;
    }
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
      const wasBotRunning = isBotRunning;
      const prevPort = monitorStatus.tcp_port;
      const prevSavedToken = savedToken.trim();
      const newBotToken = config.telegram.bot_token.trim();
      const botTokenChanged = wasBotRunning && prevSavedToken !== '' && prevSavedToken !== newBotToken;
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
          screenshot_delay_ms:
            config.notifications.screenshot_delay_ms === undefined ||
            config.notifications.screenshot_delay_ms === null ||
            (config.notifications.screenshot_delay_ms as unknown) === ''
              ? 1500
              : Number(config.notifications.screenshot_delay_ms),
        },
        startup: {
          auto_start_monitoring: !!config.startup?.auto_start_monitoring,
          start_minimized_to_tray: !!config.startup?.start_minimized_to_tray,
          close_to_tray: config.startup?.close_to_tray !== false,
        },
      };
      const result = await api.updateConfig(payload);
      config = payload;
      savedToken = payload.telegram.bot_token;
      await refreshMonitorStatus();

      if (botTokenChanged) {
        await api.stopTelegramBot();
        await new Promise((r) => setTimeout(r, 200));
        await api.startTelegramBot(payload.telegram.bot_token);
        isBotRunning = true;
        await refreshMonitorStatus();
        showStatus('Конфиг сохранён, бот перезапущен с новым токеном');
        return;
      }

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
    const token = config.telegram.bot_token.trim();
    if (!token) {
      showStatus('Введите токен бота');
      return;
    }
    try {
      await api.startTelegramBot(token);
      savedToken = token;
      isBotRunning = true;
      await refreshMonitorStatus();
      showStatus('Бот запущен');
    } catch (e) {
      showStatus(`Ошибка: ${e}`);
    }
  }

  async function restartBot() {
    const token = config.telegram.bot_token.trim();
    if (!token) {
      showStatus('Введите токен бота');
      return;
    }
    try {
      await api.stopTelegramBot();
      await new Promise((r) => setTimeout(r, 200));
      await api.startTelegramBot(token);
      savedToken = token;
      isBotRunning = true;
      await refreshMonitorStatus();
      showStatus('Бот перезапущен');
    } catch (e) {
      showStatus(`Ошибка: ${e}`);
    }
  }

  async function applyAndRestartBot() {
    const token = config.telegram.bot_token.trim();
    if (!token) {
      showStatus('Введите токен бота');
      return;
    }
    try {
      showStatus('Проверка токена...');
      await api.testTelegram(token);
      await saveConfig();
      await api.stopTelegramBot();
      await new Promise((r) => setTimeout(r, 200));
      await api.startTelegramBot(token);
      savedToken = token;
      isBotRunning = true;
      await refreshMonitorStatus();
      showStatus('Новый токен применён, бот перезапущен');
    } catch (e) {
      showStatus(`Ошибка применения токена: ${e}`);
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

  async function toggleScreenshot(chatId: string, allowed: boolean) {
    try {
      await api.setUserScreenshotAllowed(chatId, allowed);
      await refreshUsers();
      showStatus(`Скриншоты ${allowed ? 'разрешены' : 'запрещены'}`);
    } catch (e) {
      showStatus(`Ошибка: ${e}`);
    }
  }

  async function toggleStream(chatId: string, allowed: boolean) {
    try {
      await api.setUserStreamAllowed(chatId, allowed);
      await refreshUsers();
      showStatus(`Стрим ${allowed ? 'разрешён' : 'запрещён'}`);
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
    if (page === 'clicker') void refreshClickerStatus();
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
      if (currentPage === 'users') {
        void refreshUsers();
      }
    }, 2000);

    const unlisteners: Array<() => void> = [];

    void (async () => {
      try {
        const loaded = await api.getConfig();
        savedToken = loaded.telegram?.bot_token || '';
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
            screenshot_delay_ms: loaded.notifications?.screenshot_delay_ms ?? 1500,
          },
          startup: {
            auto_start_monitoring: !!loaded.startup?.auto_start_monitoring,
            start_minimized_to_tray: !!loaded.startup?.start_minimized_to_tray,
            close_to_tray: loaded.startup?.close_to_tray !== false,
          },
          updater: {
            check_on_startup: loaded.updater?.check_on_startup !== false,
            ignored_version: loaded.updater?.ignored_version ?? null,
          },
        };
        await refreshMonitorStatus();
      } catch (e) {
        console.error('Failed to load config:', e);
        lastError = String(e);
      } finally {
        configLoaded = true;
        if (config.updater?.check_on_startup !== false) {
          setTimeout(async () => {
            try {
              const update = await api.checkForUpdates();
              if (update.has_update) {
                availableUpdate = update;
                showStatus(`Доступно обновление: ${update.latest_version}`);
              }
            } catch {
              // Ignore background network errors
            }
          }, 3000);
        }
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
        unlisteners.push(
          await listen<WindowFilterMode>('window-filter-changed', (event) => {
            windowFilterMode = event.payload;
            if (event.payload.mode === 'Single') {
              showStatus(`Отслеживание: Окно #${event.payload.data.slot} (PID ${event.payload.data.pid})`);
            } else {
              showStatus('Отслеживание: Все окна');
            }
          }),
        );
        unlisteners.push(
          await listen<ClickerStatus>('clicker-status-changed', (event) => {
            clickerStatus = event.payload;
          }),
        );
        unlisteners.push(
          await listen<number>('clicker-tick', (event) => {
            clickerStatus.count = event.payload;
          }),
        );

        let isRtmpActive = false;
        let isP2pActive = false;
        const updateStreamLive = () => {
          isStreamLive = isRtmpActive || isP2pActive;
        };

        unlisteners.push(
          await listen('rtmp-status-changed', (event) => {
            isRtmpActive = Boolean(event.payload);
            updateStreamLive();
          }),
        );
        unlisteners.push(
          await listen('p2p-status-changed', (event) => {
            isP2pActive = Boolean(event.payload);
            updateStreamLive();
          }),
        );

        api.getRtmpStatus().then((s) => {
          isRtmpActive = Boolean(s);
          updateStreamLive();
        }).catch(console.warn);

        api.getP2pStatus().then((s) => {
          isP2pActive = Boolean(s);
          updateStreamLive();
        }).catch(console.warn);

        void refreshClickerStatus();
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
  <Sidebar
    {currentPage}
    {isAdmin}
    hasUpdate={!!availableUpdate?.has_update}
    updateVersion={availableUpdate?.latest_version || ''}
    {isUpdating}
    {isStreamLive}
    onNavigate={showPage}
    onDirectUpdate={handleDirectUpdate}
  />

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
        {windowFilterMode}
        {targetWindows}
        onStart={startMonitoring}
        onStop={stopMonitoring}
        onHideTray={hideToTray}
        onSelectWindow={selectWindowFilter}
        onRemoveSlot={removeWindowSlot}
        onResetWindowFilter={resetWindowFilter}
        onRefreshWindows={refreshWindowFilter}
      />
    {:else if currentPage === 'settings'}
      <SettingsPage
        bind:config
        {isBotRunning}
        {isCaptureAuto}
        {isTokenChanged}
        bind:showBotToken
        bind:availableUpdate
        bind:isUpdating
        bind:updateError
        onSave={saveConfig}
        onReset={resetConfig}
        onTestTelegram={testTelegram}
        onTestSystemNotification={testSystemNotification}
        onStartBot={startBot}
        onStopBot={stopBot}
        onRestartBot={restartBot}
        onApplyAndRestartBot={applyAndRestartBot}
      />
    {:else if currentPage === 'patterns'}
      <PatternsPage
        bind:config
        bind:newPatternName
        bind:newPatternBytes
        bind:newPatternDesc
        bind:newPatternNotifyEnabled
        bind:newPatternScreenshotEnabled
        bind:testHexInput
        {testHexResult}
        {nameFieldError}
        {bytesFieldError}
        {descFieldError}
        onTouchName={() => (patternTouched.name = true)}
        onTouchBytes={() => (patternTouched.bytes = true)}
        onTouchDesc={() => (patternTouched.desc = true)}
        onToggleNotify={togglePatternNotify}
        onToggleScreenshot={togglePatternScreenshot}
        onAdd={addPattern}
        onRemove={removePattern}
        onReset={resetPatterns}
        onExport={exportPatterns}
        onImport={importPatterns}
        onTestHex={runHexTest}
      />
    {:else if currentPage === 'clicker'}
      <ClickerPage
        {clickerStatus}
        {windowFilterMode}
        patterns={config.target.patterns}
        onStart={startClicker}
        onStop={stopClicker}
        onSave={saveClickerConfig}
        onTestKey={testSendKey}
      />
    {:else if currentPage === 'stream'}
      <StreamPage
        bind:config
        onSaveConfig={saveConfig}
      />
    {:else if currentPage === 'notifications'}
      <NotificationsPage {notifications} />
    {:else if currentPage === 'users'}
      <UsersPage
        {config}
        onRefresh={refreshUsers}
        onToggleUser={toggleUser}
        onToggleScreenshot={toggleScreenshot}
        onToggleStream={toggleStream}
        onToggleNotifyAll={toggleNotifyAll}
      />
    {:else if currentPage === 'console'}
      <ConsolePage
        {logLines}
        {packetLines}
        {packetLogEnabled}
        {targetWindows}
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

  {#if isUpdating}
    <div class="update-overlay" role="dialog" aria-modal="true">
      <div class="update-card">
        <div class="update-icon-wrapper">
          <div class="update-pulse-ring"></div>
          <div class="update-icon-circle">
            <svg class="update-icon-svg" viewBox="0 0 24 24" width="32" height="32" fill="none" stroke="currentColor" stroke-width="2.2" stroke-linecap="round" stroke-linejoin="round">
              <path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4" />
              <polyline points="7 10 12 15 17 10" />
              <line x1="12" y1="15" x2="12" y2="3" />
            </svg>
          </div>
        </div>

        <h3 class="update-title">Установка обновления</h3>

        <div class="update-version-row">
          <span class="update-ver-curr">{availableUpdate?.current_version || 'v0.4.0'}</span>
          <svg class="update-arrow-icon" viewBox="0 0 16 16" width="14" height="14" fill="currentColor">
            <path fill-rule="evenodd" d="M1 8a.5.5 0 0 1 .5-.5h11.793l-3.147-3.146a.5.5 0 0 1 .708-.708l4 4a.5.5 0 0 1 0 .708l-4 4a.5.5 0 0 1-.708-.708L13.293 8.5H1.5A.5.5 0 0 1 1 8z"/>
          </svg>
          <span class="update-ver-target">{availableUpdate?.latest_version || 'новая версия'}</span>
        </div>

        <div class="update-progress-track">
          <div class="update-progress-bar-indeterminate"></div>
        </div>

        <p class="update-desc">
          Загрузка файлов и подготовка к перезапуску...
        </p>

        <span class="update-hint">
          Программа автоматически перезапустится через пару секунд
        </span>
      </div>
    </div>
  {/if}
</div>
