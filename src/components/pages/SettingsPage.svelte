<script lang="ts">
  import { onMount } from 'svelte';
  import { listen } from '@tauri-apps/api/event';
  import PageHeader from '../PageHeader.svelte';
  import { api } from '../../lib/api';
  import type { Config, UpdateInfo, FFmpegInfo, ActiveRtmpInfo, TargetWindow, FFmpegDownloadProgress } from '../../lib/types';

  let {
    config = $bindable(),
    isBotRunning = false,
    isCaptureAuto = true,
    isTokenChanged = false,
    showBotToken = $bindable(false),
    availableUpdate = $bindable<UpdateInfo | null>(null),
    isUpdating = $bindable(false),
    updateError = $bindable<string | null>(null),
    onSave,
    onReset,
    onTestTelegram,
    onTestSystemNotification,
    onStartBot,
    onStopBot,
    onRestartBot,
    onApplyAndRestartBot,
  }: {
    config: Config;
    isBotRunning?: boolean;
    isCaptureAuto?: boolean;
    isTokenChanged?: boolean;
    showBotToken?: boolean;
    availableUpdate?: UpdateInfo | null;
    isUpdating?: boolean;
    updateError?: string | null;
    onSave: () => void;
    onReset: () => void;
    onTestTelegram: () => void;
    onTestSystemNotification?: () => void;
    onStartBot: () => void;
    onStopBot: () => void;
    onRestartBot: () => void;
    onApplyAndRestartBot?: () => void;
  } = $props();

  let currentAppVersion = $state('v0.3.15');
  let updateStatus = $state<'idle' | 'checking' | 'up-to-date' | 'available' | 'downloading' | 'error'>(
    availableUpdate?.has_update ? 'available' : 'idle'
  );
  let updateInfo = $state<UpdateInfo | null>(availableUpdate ?? null);
  let lastCheckedTime = $state<string | null>(availableUpdate ? new Date().toLocaleTimeString() : null);
  let ffmpegInfo = $state<FFmpegInfo>({ installed: false, path: null, version: null });
  let isInstallingFfmpeg = $state(false);
  let ffmpegInstallMsg = $state<string | null>(null);
  let showRtmpKey = $state(false);
  let targetWindows = $state<TargetWindow[]>([]);
  let selectedHwnd = $state<number | null>(null);
  let rtmpStreamStatus = $state<ActiveRtmpInfo | null>(null);
  let isStartingRtmp = $state(false);
  let rtmpActionError = $state<string | null>(null);
  let ffmpegDownload = $state<FFmpegDownloadProgress | null>(null);
  let isDownloadingFfmpeg = $state(false);

  async function startFfmpegDownload() {
    isDownloadingFfmpeg = true;
    ffmpegDownload = {
      downloaded_bytes: 0,
      total_bytes: null,
      percent: 0,
      stage: 'downloading',
      message: 'Подключение к официальному репозиторию FFmpeg...',
    };
    try {
      await api.downloadFFmpeg();
      ffmpegInfo = await api.getFFmpegInfo();
    } catch (e) {
      if (ffmpegDownload) {
        ffmpegDownload.stage = 'error';
        ffmpegDownload.message = 'Ошибка загрузки: ' + String(e);
      }
    } finally {
      isDownloadingFfmpeg = false;
    }
  }

  async function removeFfmpeg() {
    if (!confirm('Удалить файл ffmpeg.exe из папки программы и освободить место на диске?')) return;
    try {
      await api.deleteFFmpeg();
      ffmpegInfo = await api.getFFmpegInfo();
    } catch (e) {
      alert(String(e));
    }
  }


  let ffmpegVersionShort = $derived.by(() => {
    if (!ffmpegInfo.version) return '';
    const match = ffmpegInfo.version.match(/version\s+([0-9a-zA-Z._-]+)/i);
    return match ? match[1] : ffmpegInfo.version.split(' ')[0] || '';
  });

  $effect(() => {
    if (rtmpStreamStatus) {
      const interval = setInterval(async () => {
        try {
          rtmpStreamStatus = await api.getRtmpStatus();
        } catch {
          // ignore
        }
      }, 1000);
      return () => clearInterval(interval);
    }
  });

  async function refreshWindows() {
    try {
      targetWindows = await api.getTargetWindows();
      if (targetWindows.length > 0 && (!selectedHwnd || !targetWindows.some(w => w.hwnd === selectedHwnd))) {
        selectedHwnd = targetWindows[0].hwnd;
      }
    } catch {
      // ignore
    }
  }

  async function pasteRtmpKeyFromClipboard() {
    try {
      const clipText = await navigator.clipboard.readText();
      if (clipText && clipText.trim()) {
        if (!config.streamer) config.streamer = { rtmp_url: 'rtmps://live.me.t.me/live', rtmp_key: '' };
        config.streamer.rtmp_key = clipText.trim();
      }
    } catch (e) {
      console.warn('Clipboard read failed:', e);
    }
  }

  async function pasteRtmpUrlFromClipboard() {
    try {
      const clipText = await navigator.clipboard.readText();
      if (clipText && clipText.trim()) {
        if (!config.streamer) config.streamer = { rtmp_url: '', rtmp_key: '' };
        config.streamer.rtmp_url = clipText.trim();
      }
    } catch (e) {
      console.warn('Clipboard read failed:', e);
    }
  }

  async function startDesktopRtmp() {
    isStartingRtmp = true;
    rtmpActionError = null;
    try {
      // Re-query target windows immediately to discard closed processes
      try {
        const fresh = await api.getTargetWindows();
        if (fresh && fresh.length > 0) {
          targetWindows = fresh;
        }
      } catch (err) {
        console.warn('Auto refresh windows failed:', err);
      }

      if (targetWindows.length === 0) {
        rtmpActionError = 'Окна игры EndlessWar не найдены. Запустите игру и нажмите 🔄.';
        return;
      }

      let win = targetWindows.find(w => String(w.hwnd) === String(selectedHwnd));
      if (!win) {
        win = targetWindows[0];
        selectedHwnd = win.hwnd;
      }

      const key = (config.streamer?.rtmp_key ?? '').trim();
      if (!key) {
        rtmpActionError = 'Введите «Ключ трансляции» (Stream Key) из настроек видеотрансляции в Telegram.';
        return;
      }
      const url = (config.streamer?.rtmp_url ?? '').trim();
      rtmpStreamStatus = await api.startRtmpStream(win.hwnd, win.pid, win.title, url, key);
    } catch (e) {
      rtmpActionError = String(e);
    } finally {
      isStartingRtmp = false;
    }
  }

  async function stopDesktopRtmp() {
    try {
      await api.stopRtmpStream();
      rtmpStreamStatus = null;
    } catch (e) {
      rtmpActionError = String(e);
    }
  }

  async function installFfmpegViaWinget() {
    isInstallingFfmpeg = true;
    ffmpegInstallMsg = 'Запуск установки Gyan.FFmpeg через winget... Подождите.';
    try {
      const res = await api.installFFmpegWinget();
      ffmpegInstallMsg = res;
      ffmpegInfo = await api.getFFmpegInfo();
    } catch (e) {
      ffmpegInstallMsg = 'Ошибка: ' + String(e);
    } finally {
      isInstallingFfmpeg = false;
    }
  }

  function formatUptime(secs: number): string {
    const m = Math.floor(secs / 60);
    const s = secs % 60;
    return `${m}:${s < 10 ? '0' : ''}${s}`;
  }


  $effect(() => {
    if (isUpdating) {
      updateStatus = 'downloading';
    } else if (availableUpdate?.has_update && updateStatus !== 'downloading') {
      updateInfo = availableUpdate;
      updateStatus = 'available';
    }
  });

  onMount(async () => {
    try {
      currentAppVersion = await api.getAppVersion();
    } catch {
      // ignore
    }
    try {
      ffmpegInfo = await api.getFFmpegInfo();
      rtmpStreamStatus = await api.getRtmpStatus();
      await refreshWindows();
      const unlistenDl = await listen<FFmpegDownloadProgress>(
        'ffmpeg-download-progress',
        (event) => {
          ffmpegDownload = event.payload;
          if (event.payload.stage === 'done') {
            void api.getFFmpegInfo().then((info) => {
              ffmpegInfo = info;
            });
          }
        }
      );
    } catch {
      // ignore
    }

    if (availableUpdate?.has_update) {
      updateInfo = availableUpdate;
      updateStatus = 'available';
    } else if (config.updater?.check_on_startup !== false && updateStatus === 'idle') {
      void checkUpdates();
    }
  });

  async function checkUpdates() {
    updateStatus = 'checking';
    updateError = null;
    try {
      const info = await api.checkForUpdates();
      updateInfo = info;
      availableUpdate = info;
      lastCheckedTime = new Date().toLocaleTimeString();
      if (info.has_update) {
        updateStatus = 'available';
      } else {
        updateStatus = 'up-to-date';
      }
    } catch (e) {
      updateStatus = 'error';
      updateError = String(e);
      lastCheckedTime = new Date().toLocaleTimeString();
    }
  }

  async function installUpdate() {
    if (!updateInfo || !updateInfo.download_url || isUpdating) return;
    updateStatus = 'downloading';
    isUpdating = true;
    updateError = null;
    try {
      await api.installUpdate(updateInfo.download_url, updateInfo.asset_name);
    } catch (e) {
      updateStatus = 'error';
      isUpdating = false;
      updateError = `Ошибка установки обновления: ${e}`;
    }
  }

  function renderReleaseNotes(raw: string): string {
    if (!raw) return '';
    let s = raw
      .replace(/&/g, '&amp;')
      .replace(/</g, '&lt;')
      .replace(/>/g, '&gt;');

    // Markdown links: [text](https://...)
    s = s.replace(/\[([^\]]+)\]\((https?:\/\/[^\s)]+)\)/g, '<a href="$2" class="release-link" target="_blank" rel="noopener">$1</a>');

    // Standalone URLs (not preceded by href=")
    s = s.replace(/(^|[\s(])(https?:\/\/[^\s<)]+)/g, '$1<a href="$2" class="release-link" target="_blank" rel="noopener">$2</a>');

    // Bold: **text**
    s = s.replace(/\*\*([^*]+)\*\*/g, '<strong>$1</strong>');

    // Inline code: `code`
    s = s.replace(/`([^`]+)`/g, '<code>$1</code>');

    // Headers: ## Header or ### Header
    s = s.replace(/^###?\s+(.+)$/gm, '<div class="rn-header">$1</div>');

    // Bullets: * Item or - Item
    s = s.replace(/^[*-]\s+(.+)$/gm, '<div class="rn-item"><span class="rn-bullet">•</span> $1</div>');

    // Convert remaining newlines into <br/>
    s = s.replace(/\n/g, '<br/>');
    s = s.replace(/<\/div><br\/>/g, '</div>');
    s = s.replace(/<div class="rn-header">/g, '<div class="rn-header rn-spaced">');

    return s;
  }

  function handleReleaseNotesClick(e: MouseEvent) {
    const target = (e.target as HTMLElement).closest('a');
    if (target && target.href) {
      e.preventDefault();
      api.openUrl(target.href).catch((err) => console.error('Failed to open url:', err));
    }
  }
</script>

<PageHeader title="Настройки" />

<div class="settings-grid">
  <div class="card settings-card-telegram">
    <div class="card-header"><h2>Telegram</h2></div>
    <div class="card-body">
      <div class="form-group">
        <div class="form-label-row">
          <label for="bot-token-input">Bot Token</label>
          <button
            type="button"
            class="botfather-link-btn"
            onclick={() => api.openUrl('https://t.me/BotFather')}
            title="Открыть диалог с @BotFather в Telegram"
          >
            <svg class="tg-icon" viewBox="0 0 24 24" width="13" height="13" fill="currentColor">
              <path d="M12 2C6.48 2 2 6.48 2 12s4.48 10 10 10 10-4.48 10-10S17.52 2 12 2zm4.64 6.8c-.15 1.58-.8 5.42-1.13 7.19-.14.75-.42 1-.68 1.03-.58.05-1.02-.38-1.58-.75-.88-.58-1.38-.94-2.23-1.5-.99-.65-.35-1.01.22-1.59.15-.15 2.71-2.48 2.76-2.69a.2.2 0 00-.05-.18c-.06-.05-.14-.03-.21-.02-.09.02-1.49.95-4.22 2.79-.4.27-.76.41-1.08.4-.36-.01-1.04-.2-1.55-.37-.63-.2-1.12-.31-1.08-.66.02-.18.27-.36.74-.55 2.92-1.27 4.86-2.11 5.83-2.51 2.78-1.16 3.35-1.36 3.73-1.36.08 0 .27.02.39.12.1.08.13.19.14.27-.01.06.01.24 0 .38z"/>
            </svg>
            <span>Получить токен (@BotFather)</span>
          </button>
        </div>
        <div class="token-row">
          <input
            id="bot-token-input"
            type={showBotToken ? 'text' : 'password'}
            bind:value={config.telegram.bot_token}
            placeholder="123456789:ABCdef…"
            autocomplete="new-password"
            spellcheck="false"
          />
          <button
            type="button"
            class="btn btn-secondary btn-sm"
            onclick={() => (showBotToken = !showBotToken)}
          >
            {showBotToken ? 'Скрыть' : 'Показать'}
          </button>
        </div>
        {#if isBotRunning && isTokenChanged}
          <div class="token-notice">
            <span class="notice-icon">⚠️</span>
            <span>Токен изменён. Бот сейчас работает со старым токеном.</span>
          </div>
        {/if}
      </div>
      <div class="form-group">
        <label>Admin chat ID (для /users)</label>
        <input
          type="text"
          bind:value={config.telegram.admin_chat_id}
          placeholder="из /myid"
          autocomplete="off"
          spellcheck="false"
        />
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
      <div class="bot-status-row">
        <span class="bot-status-label">Бот:</span>
        <span class="bot-status-value" class:on={isBotRunning}>
          {isBotRunning ? 'запущен' : 'остановлен'}
        </span>
      </div>
      <div class="actions">
        <button class="btn btn-secondary" onclick={onTestTelegram}>Проверить токен</button>
        {#if isBotRunning}
          {#if isTokenChanged && onApplyAndRestartBot}
            <button class="btn btn-warning" onclick={onApplyAndRestartBot} title="Применить новый токен и перезапустить бота">
              Применить и перезапустить
            </button>
          {:else}
            <button class="btn btn-secondary" onclick={onRestartBot}>Перезапустить</button>
          {/if}
        {/if}
      </div>
      <div class="actions actions-primary">
        {#if isBotRunning}
          <button class="btn btn-danger btn-block" onclick={onStopBot}>Остановить бота</button>
        {:else}
          <button class="btn btn-success btn-block" onclick={onStartBot}>Запустить бота</button>
        {/if}
      </div>
    </div>
  </div>

  <div class="card settings-card-monitor">
    <div class="card-header"><h2>Мониторинг</h2></div>
    <div class="card-body">
      <div class="form-group">
        <label>Имя процесса</label>
        <input type="text" bind:value={config.target.process_name} placeholder="EndlessWar.exe" />
      </div>
      <div class="form-group">
        <label>Режим захвата</label>
        <div class="mode-row">
          <label class="checkbox-label">
            <input
              type="radio"
              name="capture_mode"
              checked={(config.target.capture_mode || 'auto') === 'auto'}
              onchange={() => (config.target.capture_mode = 'auto')}
            />
            <span>Авто (из процесса)</span>
          </label>
          <label class="checkbox-label">
            <input
              type="radio"
              name="capture_mode"
              checked={config.target.capture_mode === 'manual'}
              onchange={() => (config.target.capture_mode = 'manual')}
            />
            <span>Ручной (порт/IP)</span>
          </label>
        </div>
      </div>
      {#if isCaptureAuto}
        <p class="hint-text left">Remote IP/port берём из TCP-соединений процесса.</p>
      {:else}
        <div class="form-group">
          <label>TCP порт сервера (remote)</label>
          <input
            type="number"
            bind:value={config.target.tcp_port}
            min="1"
            max="65535"
            placeholder="7777"
          />
        </div>
        <div class="form-group">
          <label>IP сервера (опционально)</label>
          <input
            type="text"
            value={config.target.remote_ip ?? ''}
            oninput={(e) => {
              const v = (e.currentTarget as HTMLInputElement).value.trim();
              config.target.remote_ip = v || null;
            }}
            placeholder="пусто = любой"
            autocomplete="off"
            spellcheck="false"
          />
        </div>
      {/if}
      <div class="form-group">
        <label>Кулдаун (секунды)</label>
        <input type="number" bind:value={config.notifications.cooldown_seconds} min="0" />
      </div>
      <div class="form-group">
        <label>Burst dedup LAN+VPN (мс, 0 = выкл)</label>
        <input type="number" bind:value={config.notifications.burst_dedup_ms} min="0" step="100" />
      </div>
      <div class="form-group last">
        <label>Задержка скриншота после алерта (мс)</label>
        <input type="number" bind:value={config.notifications.screenshot_delay_ms} min="0" step="100" />
      </div>
    </div>
  </div>

  <div class="card settings-card-notify">
    <div class="card-header"><h2>Уведомления</h2></div>
    <div class="card-body">
      <div class="form-group last">
        <label class="checkbox-label">
          <input type="checkbox" bind:checked={config.notifications.tray_enabled} />
          <span>Системные уведомления Windows</span>
        </label>
        {#if onTestSystemNotification}
          <div style="margin-top: 8px;">
            <button type="button" class="btn btn-secondary btn-xs" onclick={onTestSystemNotification}>
              Проверить уведомление Windows
            </button>
          </div>
        {/if}
      </div>
    </div>
  </div>

  <div class="card settings-card-startup">
    <div class="card-header"><h2>Запуск и трей</h2></div>
    <div class="card-body">
      <div class="form-group">
        <label class="checkbox-label">
          <input type="checkbox" bind:checked={config.startup.auto_start_monitoring} />
          <span>Автозапуск мониторинга при старте приложения</span>
        </label>
      </div>
      <div class="form-group">
        <label class="checkbox-label">
          <input type="checkbox" bind:checked={config.startup.start_minimized_to_tray} />
          <span>При старте сразу сворачивать в трей</span>
        </label>
      </div>
      <div class="form-group last">
        <label class="checkbox-label">
          <input type="checkbox" bind:checked={config.startup.close_to_tray} />
          <span>Закрытие окна (×) сворачивает в трей</span>
        </label>
      </div>
    </div>
  </div>

  <div class="card settings-card-streamer">
    <div class="card-header streamer-card-header">
      <div class="streamer-header-row">
        <h2>Видеотрансляции в Telegram (RTMP)</h2>
        {#if ffmpegInfo.installed}
          <div class="installed-row">
            <span class="streamer-badge badge-success" title={ffmpegInfo.path ?? ''}>
              <span class="badge-dot dot-success"></span> FFmpeg {ffmpegVersionShort || 'готов'}
            </span>
            <button
              type="button"
              class="btn-text-danger-sm"
              onclick={removeFfmpeg}
              title="Удалить ffmpeg.exe и освободить ~200 МБ диска"
            >
              🗑 Удалить
            </button>
          </div>
        {:else}
          <span class="streamer-badge badge-warning">
            <span class="badge-dot dot-warning"></span> FFmpeg не установлен
          </span>
        {/if}
      </div>
    </div>
    <div class="card-body">
      {#if !ffmpegInfo.installed}
        <div class="ffmpeg-alert-box">
          <div class="alert-title">⚠️ <strong>FFmpeg не установлен:</strong> требуется для видеовещания в реальном времени.</div>
          <div class="alert-desc">
            Чтобы не раздувать размер релиза программы (более 200 МБ), FFmpeg не включен в дистрибутив по умолчанию и скачивается в 1 клик только теми, кому нужна трансляция.
          </div>

          {#if isDownloadingFfmpeg && ffmpegDownload}
            <div class="dl-progress-box">
              <div class="dl-progress-header">
                <span class="dl-status-text">
                  {#if ffmpegDownload.stage === 'extracting'}
                    📦 Распаковка ffmpeg.exe из архива...
                  {:else}
                    ⬇️ {ffmpegDownload.message}
                  {/if}
                </span>
                <span class="dl-percent">{Math.round(ffmpegDownload.percent)}%</span>
              </div>
              <div class="dl-progress-track">
                <div class="dl-progress-bar" style="width: {ffmpegDownload.percent}%"></div>
              </div>
            </div>
          {:else}
            {#if ffmpegDownload?.stage === 'error'}
              <div class="dl-error-msg">❌ {ffmpegDownload.message}</div>
            {/if}
            {#if ffmpegInstallMsg}
              <div class="install-msg">{ffmpegInstallMsg}</div>
            {/if}
            <div class="alert-actions">
              <button
                type="button"
                class="btn btn-sm btn-primary btn-dl-ffmpeg"
                disabled={isDownloadingFfmpeg || isInstallingFfmpeg}
                onclick={startFfmpegDownload}
              >
                ⬇️ Скачать FFmpeg (~87 МБ)
              </button>
              <button
                type="button"
                class="btn btn-sm btn-outline"
                disabled={isDownloadingFfmpeg || isInstallingFfmpeg}
                onclick={installFfmpegViaWinget}
              >
                ⚡ Winget
              </button>
              <button
                type="button"
                class="btn btn-sm btn-outline"
                onclick={() => api.openUrl('https://www.gyan.dev/ffmpeg/builds/')}
              >
                🌐 Gyan.dev
              </button>
            </div>
          {/if}
          <div class="alert-hint">Файл <code>ffmpeg.exe</code> будет сохранен прямо в папку приложения и сразу готов к работе.</div>
        </div>
      {/if}

      <!-- RTMP URL Input -->
      <div class="form-group">
        <div class="form-label-row">
          <label for="rtmp-url">Сервер трансляции Telegram (RTMP URL)</label>
          <button
            type="button"
            class="btn-text-sm"
            onclick={pasteRtmpUrlFromClipboard}
          >
            📋 Вставить
          </button>
        </div>
        <input
          id="rtmp-url"
          type="text"
          value={config.streamer?.rtmp_url ?? ''}
          oninput={(e) => {
            if (!config.streamer) config.streamer = { rtmp_url: '', rtmp_key: '' };
            config.streamer.rtmp_url = (e.target as HTMLInputElement).value;
          }}
          placeholder="rtmps://dc4-1.rtmp.t.me/s/ (или rtmps://live.me.t.me/live)"
        />
        <span class="hint">Скопируйте «URL сервера» из настроек видеотрансляции канала в Telegram (если пусто — используется по умолчанию)</span>
      </div>

      <!-- RTMP Key Input with Show/Hide and Paste -->
      <div class="form-group">
        <div class="form-label-row">
          <label for="rtmp-key">Ключ трансляции Telegram (Stream Key)</label>
          <div class="rtmp-key-buttons">
            <button
              type="button"
              class="btn-text-sm"
              onclick={() => showRtmpKey = !showRtmpKey}
            >
              {showRtmpKey ? '🙈 Скрыть' : '👁 Показать'}
            </button>
            <button
              type="button"
              class="btn-text-sm"
              onclick={pasteRtmpKeyFromClipboard}
            >
              📋 Вставить
            </button>
          </div>
        </div>
        <input
          id="rtmp-key"
          type={showRtmpKey ? 'text' : 'password'}
          value={config.streamer?.rtmp_key ?? ''}
          oninput={(e) => {
            if (!config.streamer) config.streamer = { rtmp_url: 'rtmps://live.me.t.me/live', rtmp_key: '' };
            config.streamer.rtmp_key = (e.target as HTMLInputElement).value;
          }}
          placeholder="123456789:ABCdefGHIjklMNO..."
        />
        <span class="hint">Ключ из меню вашего канала Telegram: «Видеотрансляция» ➔ «Трансляция с помощью...» ➔ «Ключ трансляции»</span>
      </div>

      <!-- Live Stream Control directly inside ew-notify Desktop UI -->
      <div class="rtmp-control-panel">
        <div class="panel-header">
          <span class="panel-title">Управление прямым эфиром из приложения</span>
          {#if rtmpStreamStatus}
            <span class="live-pill">
              <span class="live-indicator"></span> В ЭФИРЕ ({formatUptime(rtmpStreamStatus.uptime_secs)})
            </span>
          {:else}
            <span class="idle-pill">Остановлено</span>
          {/if}
        </div>

        {#if rtmpActionError}
          <div class="rtmp-error-msg">⚠️ {rtmpActionError}</div>
        {/if}

        <div class="panel-body">
          {#if rtmpStreamStatus}
            <div class="live-status-details">
              🎮 Окно: <strong>{rtmpStreamStatus.title}</strong> (PID {rtmpStreamStatus.pid}) | 🚀 720p HD 30 FPS (без звука)
            </div>
            <button
              type="button"
              class="btn btn-danger btn-sm"
              onclick={stopDesktopRtmp}
            >
              🛑 Остановить вещание в Telegram
            </button>
          {:else}
            <div class="launch-row">
              <select
                class="window-select"
                bind:value={selectedHwnd}
                disabled={targetWindows.length === 0}
              >
                {#if targetWindows.length === 0}
                  <option value="">Окна игры не найдены (запустите EndlessWar)</option>
                {:else}
                  {#each targetWindows as win, idx}
                    <option value={win.hwnd}>
                      🎮 Окно #{idx + 1} — {win.title} (PID {win.pid}){win.is_minimized ? ' [Свёрнуто]' : ''}
                    </option>
                  {/each}
                {/if}
              </select>
              <button
                type="button"
                class="btn btn-refresh-sm"
                onclick={refreshWindows}
                title="Обновить список окон"
              >
                🔄
              </button>
              <button
                type="button"
                class="btn btn-primary btn-sm"
                disabled={!selectedHwnd || !ffmpegInfo.installed || !config.streamer?.rtmp_key || isStartingRtmp}
                onclick={startDesktopRtmp}
              >
                {isStartingRtmp ? 'Запуск...' : '▶️ Начать эфир в Telegram'}
              </button>
            </div>
          {/if}
        </div>
      </div>

      <!-- Quick Setup Guide -->
      <div class="streamer-guide">
        <div class="guide-header">📖 <strong>Пошаговая инструкция по запуску трансляции в Telegram:</strong></div>
        <ol class="guide-list">
          <li>Создайте закрытый канал или группу в Telegram (куда будет идти трансляция).</li>
          <li>В Telegram нажмите на меню канала ➔ <strong>«Видеотрансляция»</strong> (или «Прямой эфир») ➔ <strong>«Трансляция с помощью стороннего ПО»</strong>.</li>
          <li>Telegram выдаст два параметра:
            <br/>• <strong>URL сервера:</strong> скопируйте и нажмите <strong>«📋 Вставить»</strong> в поле «Сервер трансляции» выше.
            <br/>• <strong>Ключ трансляции:</strong> скопируйте и нажмите <strong>«📋 Вставить»</strong> в поле «Ключ трансляции» выше.
          </li>
          <li>Нажмите <strong>«Сохранить настройки»</strong> внизу страницы (ключ постоянный и сохраняется навсегда).</li>
          <li>Выберите окно игры выше и нажмите <strong>«▶️ Начать эфир в Telegram»</strong> (или нажмите кнопку <strong>📡 RTMP</strong> в Telegram-боте).</li>
          <li><strong>ВАЖНО:</strong> в окне видеотрансляции канала в Telegram нажмите синюю кнопку <strong>«Начать трансляцию»</strong> — после этого прямой эфир игры появится на экране!</li>
        </ol>
      </div>
    </div>
  </div>

  <div class="card settings-card-updater">
    <div class="card-header update-card-header">
      <h2>Обновление программы</h2>
      <span class="version-badge">{currentAppVersion || 'v0.3.15'}</span>
    </div>
    <div class="card-body">
      <div class="form-group">
        <label class="checkbox-label">
          <input
            type="checkbox"
            checked={config.updater ? config.updater.check_on_startup : true}
            onchange={(e) => {
              if (!config.updater) config.updater = { check_on_startup: true, ignored_version: null };
              config.updater.check_on_startup = (e.target as HTMLInputElement).checked;
            }}
          />
          <span>Проверять наличие обновлений при запуске приложения</span>
        </label>
      </div>

      <div class="update-actions">
        <button
          type="button"
          class="btn btn-secondary btn-sm"
          disabled={updateStatus === 'checking' || updateStatus === 'downloading'}
          onclick={checkUpdates}
        >
          {updateStatus === 'checking' ? 'Проверка...' : 'Проверить обновления'}
        </button>

        {#if lastCheckedTime}
          <span class="last-checked">Проверено: {lastCheckedTime}</span>
        {/if}
      </div>

      {#if updateStatus === 'up-to-date'}
        <div class="update-notice success">
          <span class="notice-icon">✓</span>
          <span>У вас установлена самая актуальная версия программы.</span>
        </div>
      {:else if updateStatus === 'error'}
        <div class="update-notice error">
          <span class="notice-icon">⚠</span>
          <span>{updateError || 'Ошибка проверки обновлений'}</span>
        </div>
      {:else if updateStatus === 'available' && updateInfo}
        <div class="update-card-details">
          <div class="update-header">
            <div>
              <div class="update-tag">Доступна версия {updateInfo.latest_version}</div>
              {#if updateInfo.published_at}
                <div class="update-date">Опубликовано: {new Date(updateInfo.published_at).toLocaleDateString()}</div>
              {/if}
            </div>
            <button
              type="button"
              class="btn btn-primary btn-sm btn-update"
              disabled={updateStatus === 'downloading'}
              onclick={installUpdate}
            >
              Скачать и обновить
            </button>
          </div>

          {#if updateInfo.release_notes}
            <div class="release-notes-box">
              <div class="release-notes-title">Что нового в обновлении:</div>
              <!-- svelte-ignore a11y_click_events_have_key_events -->
              <!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
              <div
                class="release-notes-content"
                onclick={handleReleaseNotesClick}
                role="region"
                aria-label="Заметки к обновлению"
              >
                {@html renderReleaseNotes(updateInfo.release_notes)}
              </div>
            </div>
          {/if}
        </div>
      {:else if updateStatus === 'downloading'}
        <div class="update-notice downloading">
          <div class="spinner"></div>
          <span>Скачивание обновления и замена файлов... Приложение автоматически перезапустится.</span>
        </div>
      {/if}
    </div>
  </div>

  <div class="actions settings-footer">
    <button class="btn btn-primary" onclick={onSave}>Сохранить настройки</button>
    <button type="button" class="btn btn-danger" onclick={onReset}>Сбросить</button>
  </div>
</div>

<style>
  .form-label-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    margin-bottom: 6px;
  }

  .form-label-row label {
    margin-bottom: 0;
  }

  .botfather-link-btn {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    background: rgba(36, 161, 222, 0.12);
    border: 1px solid rgba(36, 161, 222, 0.35);
    border-radius: 4px;
    color: #38bdf8;
    font-size: 11px;
    font-weight: 500;
    padding: 3px 8px;
    cursor: pointer;
    transition: all 0.15s ease;
  }

  .botfather-link-btn:hover {
    background: rgba(36, 161, 222, 0.25);
    border-color: #38bdf8;
    color: #fff;
    transform: translateY(-1px);
  }

  .botfather-link-btn .tg-icon {
    flex-shrink: 0;
  }

  .token-notice {
    display: flex;
    align-items: center;
    gap: 6px;
    margin-top: 8px;
    font-size: 12px;
    color: var(--warn, #f0c674);
    background: rgba(240, 198, 116, 0.1);
    padding: 6px 10px;
    border-radius: 6px;
    border: 1px solid rgba(240, 198, 116, 0.25);
  }

  .btn-warning {
    background: #d97706;
    color: #fff;
  }
  .btn-warning:hover {
    background: #b45309;
  }

  .update-card-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
  }

  .version-badge {
    font-size: 11px;
    font-weight: 700;
    color: var(--accent, #60a5fa);
    background: rgba(96, 165, 250, 0.12);
    padding: 2px 8px;
    border-radius: 999px;
    border: 1px solid rgba(96, 165, 250, 0.25);
  }

  .update-actions {
    display: flex;
    align-items: center;
    gap: 12px;
    margin-top: 10px;
    margin-bottom: 8px;
  }

  .last-checked {
    font-size: 11px;
    color: var(--text-muted, #888);
  }

  .update-notice {
    display: flex;
    align-items: center;
    gap: 8px;
    margin-top: 12px;
    padding: 10px 14px;
    border-radius: 6px;
    font-size: 13px;
  }

  .update-notice.success {
    background: rgba(34, 197, 94, 0.1);
    color: #4ade80;
    border: 1px solid rgba(34, 197, 94, 0.25);
  }

  .update-notice.error {
    background: rgba(239, 68, 68, 0.1);
    color: #f87171;
    border: 1px solid rgba(239, 68, 68, 0.25);
  }

  .update-notice.downloading {
    background: rgba(96, 165, 250, 0.1);
    color: #93c5fd;
    border: 1px solid rgba(96, 165, 250, 0.25);
  }

  .update-card-details {
    margin-top: 12px;
    padding: 14px;
    background: rgba(255, 255, 255, 0.03);
    border: 1px solid rgba(96, 165, 250, 0.3);
    border-radius: 8px;
  }

  .update-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    margin-bottom: 12px;
  }

  .update-tag {
    font-size: 15px;
    font-weight: 700;
    color: var(--accent, #60a5fa);
  }

  .update-date {
    font-size: 11px;
    color: var(--text-muted, #888);
    margin-top: 2px;
  }

  .btn-update {
    background: #2563eb;
    color: #fff;
    font-weight: 600;
  }
  .btn-update:hover {
    background: #1d4ed8;
  }

  .release-notes-box {
    margin-top: 10px;
    padding: 10px 12px;
    background: rgba(0, 0, 0, 0.25);
    border-radius: 6px;
    border: 1px solid rgba(255, 255, 255, 0.06);
  }

  .release-notes-title {
    font-size: 12px;
    font-weight: 600;
    color: var(--text-muted, #aaa);
    margin-bottom: 6px;
  }

  .release-notes-content {
    font-family: inherit;
    font-size: 12px;
    line-height: 1.5;
    color: var(--text-normal, #ddd);
    word-break: break-word;
    margin: 0;
    max-height: 160px;
    overflow-y: auto;
  }

  :global(.release-notes-content a.release-link) {
    color: var(--accent, #6366f1);
    text-decoration: underline;
    text-underline-offset: 2px;
    cursor: pointer;
    transition: color 0.15s ease;
  }

  :global(.release-notes-content a.release-link:hover) {
    color: #818cf8;
  }

  :global(.release-notes-content strong) {
    color: #fff;
    font-weight: 600;
  }

  :global(.release-notes-content .rn-header) {
    font-weight: 600;
    color: #fff;
    margin-top: 4px;
    margin-bottom: 2px;
  }

  :global(.release-notes-content .rn-header.rn-spaced) {
    margin-top: 8px;
  }

  :global(.release-notes-content .rn-item) {
    padding-left: 2px;
    margin: 2px 0;
  }

  :global(.release-notes-content .rn-bullet) {
    color: var(--accent, #6366f1);
    margin-right: 4px;
  }

  :global(.release-notes-content code) {
    padding: 1px 4px;
    border-radius: 4px;
    background: rgba(255, 255, 255, 0.08);
    font-size: 11px;
  }

  .spinner {
    width: 16px;
    height: 16px;
    border: 2px solid rgba(255, 255, 255, 0.2);
    border-top-color: #60a5fa;
    border-radius: 50%;
    animation: spin 0.8s linear infinite;
    flex-shrink: 0;
  }

  @keyframes spin {
    to {
      transform: rotate(360deg);
    }
  }

  .streamer-header-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    width: 100%;
  }

  .streamer-badge {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    font-size: 11px;
    font-weight: 600;
    padding: 3px 10px;
    border-radius: 999px;
  }

  .streamer-badge.badge-success {
    background: rgba(34, 197, 94, 0.15);
    border: 1px solid rgba(34, 197, 94, 0.35);
    color: #4ade80;
  }

  .streamer-badge.badge-warning {
    background: rgba(234, 179, 8, 0.15);
    border: 1px solid rgba(234, 179, 8, 0.35);
    color: #facc15;
  }

  .badge-dot {
    width: 7px;
    height: 7px;
    border-radius: 50%;
  }

  .badge-dot.dot-success {
    background: #22c55e;
    box-shadow: 0 0 6px rgba(34, 197, 94, 0.6);
  }

  .badge-dot.dot-warning {
    background: #eab308;
    box-shadow: 0 0 6px rgba(234, 179, 8, 0.6);
  }

  .ffmpeg-alert-box {
    margin-bottom: 16px;
    padding: 12px 14px;
    background: rgba(234, 179, 8, 0.08);
    border: 1px solid rgba(234, 179, 8, 0.25);
    border-radius: 6px;
    font-size: 13px;
    color: #fef08a;
  }

  .alert-actions {
    display: flex;
    gap: 8px;
    margin-top: 10px;
    margin-bottom: 8px;
  }

  .alert-hint {
    font-size: 11px;
    opacity: 0.8;
  }

  .install-msg {
    margin-top: 6px;
    font-size: 12px;
    color: #38bdf8;
  }

  .rtmp-key-buttons {
    display: flex;
    gap: 8px;
  }

  .btn-text-sm {
    background: none;
    border: 1px solid rgba(255, 255, 255, 0.15);
    color: #38bdf8;
    font-size: 11px;
    padding: 2px 7px;
    border-radius: 4px;
    cursor: pointer;
  }

  .btn-text-sm:hover {
    background: rgba(56, 189, 248, 0.15);
    border-color: #38bdf8;
  }

  .rtmp-control-panel {
    margin-top: 16px;
    margin-bottom: 16px;
    padding: 14px;
    background: rgba(15, 23, 42, 0.6);
    border: 1px solid rgba(56, 189, 248, 0.25);
    border-radius: 8px;
  }

  .panel-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    margin-bottom: 12px;
  }

  .panel-title {
    font-size: 13px;
    font-weight: 600;
    color: #e2e8f0;
  }

  .live-pill {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    background: rgba(239, 68, 68, 0.2);
    border: 1px solid rgba(239, 68, 68, 0.4);
    color: #f87171;
    font-size: 11px;
    font-weight: 700;
    padding: 2px 8px;
    border-radius: 999px;
  }

  .live-indicator {
    width: 6px;
    height: 6px;
    border-radius: 50%;
    background: #ef4444;
    box-shadow: 0 0 6px #ef4444;
    animation: pulse-live 1.5s infinite;
  }

  @keyframes pulse-live {
    0%, 100% { opacity: 1; transform: scale(1); }
    50% { opacity: 0.4; transform: scale(0.85); }
  }

  .idle-pill {
    font-size: 11px;
    color: #94a3b8;
    background: rgba(255, 255, 255, 0.05);
    padding: 2px 8px;
    border-radius: 999px;
  }

  .launch-row {
    display: flex;
    gap: 8px;
    align-items: center;
  }

  .window-select {
    flex: 1;
    background: #0f172a;
    border: 1px solid rgba(255, 255, 255, 0.15);
    border-radius: 6px;
    padding: 6px 10px;
    color: #f8fafc;
    font-size: 13px;
  }

  .btn-refresh-sm {
    background: rgba(255, 255, 255, 0.08);
    border: 1px solid rgba(255, 255, 255, 0.15);
    border-radius: 6px;
    padding: 6px 10px;
    cursor: pointer;
  }

  .btn-refresh-sm:hover {
    background: rgba(255, 255, 255, 0.15);
  }

  .live-status-details {
    font-size: 13px;
    color: #cbd5e1;
    margin-bottom: 10px;
  }

  .rtmp-error-msg {
    padding: 6px 10px;
    background: rgba(239, 68, 68, 0.15);
    border: 1px solid rgba(239, 68, 68, 0.3);
    border-radius: 4px;
    color: #fca5a5;
    font-size: 12px;
    margin-bottom: 10px;
  }

  .streamer-guide {
    padding: 12px 14px;
    background: rgba(56, 189, 248, 0.06);
    border: 1px solid rgba(56, 189, 248, 0.18);
    border-radius: 6px;
    font-size: 12px;
    color: #bae6fd;
    line-height: 1.5;
  }

  .guide-header {
    margin-bottom: 6px;
    color: #38bdf8;
    font-size: 13px;
  }

  .guide-list {
    margin: 0;
    padding-left: 20px;
  }

  .guide-list li {
    margin-bottom: 4px;
  }
  .installed-row {
    display: flex;
    align-items: center;
    gap: 8px;
  }

  .dl-progress-box {
    margin-top: 10px;
    margin-bottom: 10px;
    padding: 10px 12px;
    background: rgba(15, 23, 42, 0.7);
    border: 1px solid rgba(56, 189, 248, 0.3);
    border-radius: 6px;
  }

  .dl-progress-header {
    display: flex;
    justify-content: space-between;
    font-size: 12px;
    color: #38bdf8;
    margin-bottom: 6px;
    font-weight: 500;
  }

  .dl-percent {
    font-weight: 700;
    color: #4ade80;
  }

  .dl-progress-track {
    width: 100%;
    height: 8px;
    background: rgba(255, 255, 255, 0.1);
    border-radius: 4px;
    overflow: hidden;
  }

  .dl-progress-bar {
    height: 100%;
    background: linear-gradient(90deg, #38bdf8, #22c55e);
    transition: width 0.2s ease;
  }

  .dl-error-msg {
    margin-top: 8px;
    font-size: 12px;
    color: #f87171;
  }

  .btn-dl-ffmpeg {
    font-weight: 600;
    background: #0284c7;
  }

  .btn-dl-ffmpeg:hover {
    background: #0369a1;
  }

  .btn-text-danger-sm {
    background: none;
    border: 1px solid rgba(239, 68, 68, 0.3);
    color: #f87171;
    font-size: 11px;
    padding: 2px 7px;
    border-radius: 4px;
    cursor: pointer;
  }

  .btn-text-danger-sm:hover {
    background: rgba(239, 68, 68, 0.15);
    border-color: #ef4444;
  }
</style>
