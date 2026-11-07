<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import { listen, type UnlistenFn } from '@tauri-apps/api/event';
  import { api } from '../../lib/api';
  import type {
    Config,
    ActiveRtmpInfo,
    FFmpegInfo,
    FFmpegDownloadProgress,
    TargetWindow,
    UpnpStatusResult,
  } from '../../lib/types';

  let {
    config = $bindable(),
    onSaveConfig,
  }: {
    config: Config;
    onSaveConfig?: () => Promise<void>;
  } = $props();

  let ffmpegInfo = $state<FFmpegInfo>({ installed: false, path: null, version: null });
  let ffmpegInstallMsg = $state<string | null>(null);
  let isInstallingFfmpeg = $state(false);
  let showRtmpKey = $state(false);
  let targetWindows = $state<TargetWindow[]>([]);
  let selectedHwnd = $state<number | null>(null);
  let rtmpStreamStatus = $state<ActiveRtmpInfo | null>(null);
  let isStartingRtmp = $state(false);
  let rtmpActionError = $state<string | null>(null);
  let isTestingRtmp = $state(false);
  let rtmpTestResult = $state<{ ok: boolean; message: string } | null>(null);
  let ffmpegDownload = $state<FFmpegDownloadProgress | null>(null);
  let isDownloadingFfmpeg = $state(false);
  let isCheckingUpnp = $state(false);
  let upnpResult = $state<UpnpStatusResult | null>(null);
  let isSaving = $state(false);
  let saveSuccess = $state(false);
  let copySuccess = $state(false);

  let unlistenFfmpeg: UnlistenFn | null = null;
  let unlistenStatus: UnlistenFn | null = null;
  let pollTimer: ReturnType<typeof setInterval> | null = null;

  const isRtmpConfigured = $derived(
    Boolean(
      (config.streamer?.rtmp_key ?? '').trim().length > 0 &&
      (config.streamer?.rtmp_url ?? '').trim().length > 0
    )
  );

  const selectedResolution = $derived(config.streamer?.rtmp_resolution ?? '1080p');

  let ffmpegVersionShort = $derived.by(() => {
    if (!ffmpegInfo.version) return '';
    const match = ffmpegInfo.version.match(/version\s+([0-9a-zA-Z._-]+)/i);
    return match ? match[1] : ffmpegInfo.version.split(' ')[0] || '';
  });

  onMount(async () => {
    try {
      ffmpegInfo = await api.getFFmpegInfo();
    } catch (e) {
      console.warn('Failed to get FFmpeg info:', e);
    }

    try {
      rtmpStreamStatus = await api.getRtmpStatus();
    } catch (e) {
      console.warn('Failed to get RTMP status:', e);
    }

    await refreshWindows();

    // Industry standard: asynchronous background UPnP check on startup/mount
    void checkUpnp();

    // 1. Continuous status poller: polls every 1000ms REGARDLESS of whether stream is active or not!
    // Ensures stream started from Telegram bot is immediately reflected in ew-notify UI
    pollTimer = setInterval(async () => {
      try {
        const next = await api.getRtmpStatus();
        const wasStreaming = rtmpStreamStatus != null;
        const isNowStreaming = next != null;
        rtmpStreamStatus = next;
        if (!wasStreaming && isNowStreaming) {
          void refreshWindows();
        }
      } catch {
        // ignore
      }
    }, 1000);

    // 2. Realtime Tauri event listener for instant 0ms sync
    try {
      unlistenStatus = await listen<ActiveRtmpInfo | null>('rtmp-status-changed', (event) => {
        rtmpStreamStatus = event.payload;
        if (event.payload) {
          void refreshWindows();
        }
      });
    } catch (e) {
      console.warn('Failed to listen to rtmp-status-changed:', e);
    }

    // 3. Listen to FFmpeg download progress
    try {
      unlistenFfmpeg = await listen<FFmpegDownloadProgress>('ffmpeg-download-progress', (event) => {
        ffmpegDownload = event.payload;
        if (event.payload.stage === 'ready' || event.payload.stage === 'done') {
          isDownloadingFfmpeg = false;
          api.getFFmpegInfo().then((info) => (ffmpegInfo = info)).catch(console.warn);
        } else if (event.payload.stage === 'error') {
          isDownloadingFfmpeg = false;
        }
      });
    } catch (e) {
      console.warn('Failed to listen to ffmpeg progress:', e);
    }
  });

  onDestroy(() => {
    if (pollTimer) clearInterval(pollTimer);
    if (unlistenStatus) unlistenStatus();
    if (unlistenFfmpeg) unlistenFfmpeg();
  });

  async function refreshWindows() {
    try {
      targetWindows = await api.getTargetWindows();
      if (targetWindows.length > 0) {
        if (!selectedHwnd || !targetWindows.some((w) => w.hwnd === selectedHwnd)) {
          selectedHwnd = targetWindows[0].hwnd;
        }
      } else {
        selectedHwnd = null;
      }
    } catch (e) {
      console.warn('Failed to list target windows:', e);
    }
  }

  async function saveSettings() {
    isSaving = true;
    saveSuccess = false;
    try {
      if (onSaveConfig) {
        await onSaveConfig();
      } else {
        await api.saveConfig(config);
      }
      saveSuccess = true;
      setTimeout(() => (saveSuccess = false), 2500);
    } catch (e) {
      alert('Ошибка сохранения настроек: ' + String(e));
    } finally {
      isSaving = false;
    }
  }

  function setResolution(res: '1080p' | '720p') {
    if (!config.streamer) {
      config.streamer = {
        rtmp_url: 'rtmps://live.me.t.me/live',
        rtmp_key: '',
        rtmp_channel_url: '',
        rtmp_resolution: res,
      };
    } else {
      config.streamer.rtmp_resolution = res;
    }
  }

  async function pasteRtmpUrlFromClipboard() {
    try {
      const clipText = await navigator.clipboard.readText();
      if (clipText && clipText.trim()) {
        if (!config.streamer) config.streamer = { rtmp_url: '', rtmp_key: '', rtmp_channel_url: '' };
        config.streamer.rtmp_url = clipText.trim();
      }
    } catch (e) {
      console.warn('Clipboard read failed:', e);
    }
  }

  async function pasteRtmpKeyFromClipboard() {
    try {
      const clipText = await navigator.clipboard.readText();
      if (clipText && clipText.trim()) {
        if (!config.streamer) config.streamer = { rtmp_url: '', rtmp_key: '', rtmp_channel_url: '' };
        config.streamer.rtmp_key = clipText.trim();
      }
    } catch (e) {
      console.warn('Clipboard read failed:', e);
    }
  }

  async function pasteChannelUrlFromClipboard() {
    try {
      const clipText = await navigator.clipboard.readText();
      if (clipText && clipText.trim()) {
        if (!config.streamer) config.streamer = { rtmp_url: '', rtmp_key: '', rtmp_channel_url: '' };
        config.streamer.rtmp_channel_url = clipText.trim();
      }
    } catch (e) {
      console.warn('Clipboard read failed:', e);
    }
  }

  async function copyChannelUrl() {
    const url = config.streamer?.rtmp_channel_url?.trim();
    if (!url) return;
    try {
      await navigator.clipboard.writeText(url);
      copySuccess = true;
      setTimeout(() => (copySuccess = false), 2000);
    } catch (e) {
      console.warn('Clipboard write error:', e);
    }
  }

  async function testRtmpConnection() {
    const url = (config.streamer?.rtmp_url ?? '').trim();
    const key = (config.streamer?.rtmp_key ?? '').trim();
    if (!key) {
      rtmpTestResult = { ok: false, message: 'Сначала укажите «Ключ трансляции» (Stream Key).' };
      return;
    }
    isTestingRtmp = true;
    rtmpTestResult = null;
    try {
      const msg = await api.testRtmpConnection(url, key);
      rtmpTestResult = { ok: true, message: `✅ ${msg}` };
    } catch (e) {
      rtmpTestResult = { ok: false, message: `❌ ${String(e)}` };
    } finally {
      isTestingRtmp = false;
    }
  }

  async function switchStreamWindow(win: TargetWindow) {
    selectedHwnd = win.hwnd;
    isStartingRtmp = true;
    rtmpActionError = null;
    try {
      const key = (config.streamer?.rtmp_key ?? '').trim();
      if (!key) {
        rtmpActionError = 'Ключ трансляции не указан в настройках ниже. Заполните «Ключ трансляции» и нажмите Сохранить.';
        return;
      }
      if (rtmpStreamStatus) {
        await api.stopRtmpStream();
        await new Promise((r) => setTimeout(r, 200));
      }
      const url = (config.streamer?.rtmp_url ?? '').trim();
      const chan = (config.streamer?.rtmp_channel_url ?? '').trim();
      const res = config.streamer?.rtmp_resolution ?? '1080p';
      rtmpStreamStatus = await api.startRtmpStream(win.hwnd, win.pid, win.title, url, key, chan, res);
    } catch (e) {
      rtmpActionError = String(e);
    } finally {
      isStartingRtmp = false;
    }
  }

  async function restartStream() {
    if (!selectedHwnd) return;
    const win = targetWindows.find((w) => w.hwnd === selectedHwnd);
    if (win) {
      await switchStreamWindow(win);
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

  async function installFfmpegViaWinget() {
    isInstallingFfmpeg = true;
    ffmpegInstallMsg = 'Запуск установки FFmpeg через winget...';
    try {
      const res = await api.installFFmpegWinget();
      ffmpegInstallMsg = res;
      ffmpegInfo = await api.getFFmpegInfo();
    } catch (e) {
      ffmpegInstallMsg = 'Ошибка winget: ' + String(e);
    } finally {
      isInstallingFfmpeg = false;
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

  async function checkUpnp() {
    isCheckingUpnp = true;
    try {
      upnpResult = await api.checkUpnpStatus();
    } catch (e) {
      upnpResult = {
        available: false,
        external_ip: null,
        active_port: 0,
        details: 'Ошибка проверки UPnP: ' + String(e),
      };
    } finally {
      isCheckingUpnp = false;
    }
  }

  function formatUptime(secs: number): string {
    const m = Math.floor(secs / 60);
    const s = secs % 60;
    return `${m}:${s.toString().padStart(2, '0')}`;
  }
</script>

<div class="stream-page">
  <div class="page-header">
    <div class="header-content">
      <h1>📺 Стриминг и видеотрансляции</h1>
      <p class="subtitle">
        Трансляция игрового окна в реальном времени прямо в Telegram (RTMP в закрытый канал или P2P WebRTC в Mini App).
      </p>
    </div>
  </div>

  <!-- SECTION 1: RTMP Telegram Streaming & Management -->
  <div class="card stream-card">
    <div class="card-header">
      <div class="header-left">
        <h2>📡 Видеотрансляция в Telegram (RTMP)</h2>
        <span class="recommended-pill">Рекомендуемый способ</span>
      </div>
      <div class="header-right">
        {#if ffmpegInfo.installed}
          <span class="badge badge-success" title={ffmpegInfo.path ?? ''}>
            <span class="dot dot-success"></span> FFmpeg {ffmpegVersionShort || 'готов'}
          </span>
          <button
            type="button"
            class="btn-text-danger-sm"
            onclick={removeFfmpeg}
            title="Удалить ffmpeg.exe и освободить ~200 МБ диска"
          >
            🗑
          </button>
        {:else}
          <span class="badge badge-warning">
            <span class="dot dot-warning"></span> FFmpeg не установлен
          </span>
        {/if}
      </div>
    </div>

    <div class="card-body">
      <!-- FFmpeg Alert if not installed -->
      {#if !ffmpegInfo.installed}
        <div class="ffmpeg-alert-box">
          <div class="alert-title">⚠️ <strong>Для видеовещания требуется компонент FFmpeg</strong></div>
          <div class="alert-desc">
            FFmpeg кодирует видеопоток игры в формат H.264 с аппаратным ускорением и отправляет в Telegram. Нажмите кнопку ниже, чтобы автоматически скачать его в 1 клик (~87 МБ).
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
                ⚡ Установить через Winget
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
        </div>
      {/if}

      <!-- ================= STREAM MANAGER PANEL ================= -->
      <div class="stream-manager-card">
        <div class="manager-header">
          <div class="manager-title-group">
            <h3>🎮 Менеджер видеотрансляции EndlessWar</h3>
            <span class="manager-sub">Управление активным захватом окон и прямым эфиром</span>
          </div>
          <div class="manager-header-actions">
            {#if rtmpStreamStatus}
              <span class="live-pill">
                <span class="live-dot-pulse"></span> В ЭФИРЕ ({formatUptime(rtmpStreamStatus.uptime_secs)})
              </span>
            {:else}
              <span class="offline-pill">ОФФЛАЙН</span>
            {/if}
            <button
              type="button"
              class="btn-refresh-icon"
              onclick={refreshWindows}
              title="Обновить список открытых окон игры"
            >
              🔄
            </button>
          </div>
        </div>

        {#if rtmpActionError}
          <div class="rtmp-error-msg">⚠️ {rtmpActionError}</div>
        {/if}

        <!-- Active Live Stream Details Card -->
        {#if rtmpStreamStatus}
          <div class="active-session-banner">
            <div class="session-main">
              <div class="session-game-title">
                <span class="game-tag">ИГРА</span>
                <strong>{rtmpStreamStatus.title}</strong>
                <span class="pid-tag">PID: {rtmpStreamStatus.pid}</span>
              </div>
              <div class="session-specs-row">
                <span class="spec-item">⏱ В эфире: <strong>{formatUptime(rtmpStreamStatus.uptime_secs)}</strong></span>
                <span class="spec-item">📺 Качество: <strong>{rtmpStreamStatus.resolution} (30 FPS)</strong></span>
                <span class="spec-item">⚡ Битрейт: <strong>{rtmpStreamStatus.resolution.includes('720') ? '~2500 kbps' : '~4500 kbps'}</strong></span>
                <span class="spec-item">📡 Кодек: <strong>H.264 (FFmpeg)</strong></span>
              </div>
            </div>
            <div class="session-action-btns">
              {#if config.streamer?.rtmp_channel_url}
                <button
                  type="button"
                  class="btn btn-sm btn-outline-channel"
                  onclick={() => api.openUrl(config.streamer?.rtmp_channel_url || '')}
                  title="Открыть Telegram-канал с трансляцией"
                >
                  📢 Открыть канал
                </button>
                <button
                  type="button"
                  class="btn btn-sm btn-outline"
                  onclick={copyChannelUrl}
                  title="Скопировать ссылку на канал"
                >
                  {copySuccess ? '✅ Скопировано!' : '📋 Ссылка'}
                </button>
              {/if}
              <button
                type="button"
                class="btn btn-sm btn-restart"
                onclick={restartStream}
                disabled={isStartingRtmp}
                title="Перезапустить захват видео"
              >
                🔄 Перезапуск
              </button>
              <button
                type="button"
                class="btn btn-sm btn-danger"
                onclick={stopDesktopRtmp}
              >
                🛑 Остановить эфир
              </button>
            </div>
          </div>
        {:else}
          <!-- Pre-flight Status Checklist -->
          <div class="preflight-bar">
            <div class="preflight-item" class:ok={ffmpegInfo.installed}>
              <span class="pf-icon">{ffmpegInfo.installed ? '✅' : '⚠️'}</span>
              <span>FFmpeg: <strong>{ffmpegInfo.installed ? 'Готов' : 'Не установлен'}</strong></span>
            </div>
            <div class="preflight-item" class:ok={isRtmpConfigured}>
              <span class="pf-icon">{isRtmpConfigured ? '✅' : '⚠️'}</span>
              <span>Ключ RTMP: <strong>{isRtmpConfigured ? 'Сохранён' : 'Не указан'}</strong></span>
            </div>
            <div class="preflight-item" class:ok={targetWindows.length > 0}>
              <span class="pf-icon">{targetWindows.length > 0 ? '✅' : '⚠️'}</span>
              <span>Окна EndlessWar: <strong>{targetWindows.length > 0 ? `${targetWindows.length} обнаружено` : 'Не найдены'}</strong></span>
            </div>
          </div>
        {/if}

        <!-- Game Windows Table / Grid -->
        <div class="game-windows-section">
          <div class="windows-section-title">
            <span>🎮 Список окон клиента EndlessWar:</span>
            <span class="windows-count">Найдено: {targetWindows.length}</span>
          </div>

          {#if targetWindows.length === 0}
            <div class="empty-windows-box">
              <span class="empty-icon">🎮</span>
              <div class="empty-text">
                Окна игры EndlessWar не найдены. Запустите клиент игры и нажмите <strong>🔄 Обновить</strong>.
              </div>
              <button type="button" class="btn btn-sm btn-outline" onclick={refreshWindows}>
                🔄 Обновить список окон
              </button>
            </div>
          {:else}
            <div class="windows-list">
              {#each targetWindows as win, idx}
                {@const isCurrentlyStreaming = rtmpStreamStatus && String(rtmpStreamStatus.hwnd) === String(win.hwnd)}
                <div class="window-item-card" class:is-streaming={isCurrentlyStreaming}>
                  <div class="win-left-info">
                    <span class="slot-badge">#{idx + 1}</span>
                    <div class="win-meta">
                      <div class="win-title-row">
                        <strong class="win-title">{win.title}</strong>
                        {#if win.is_minimized}
                          <span class="minimized-tag">Свёрнуто</span>
                        {/if}
                      </div>
                      <span class="win-pid-info">PID: {win.pid} • HWND: {win.hwnd}</span>
                    </div>
                  </div>

                  <div class="win-right-actions">
                    {#if isCurrentlyStreaming}
                      <span class="current-live-tag">
                        <span class="live-dot-pulse"></span> ИДЁТ ЭФИР
                      </span>
                      <button
                        type="button"
                        class="btn btn-sm btn-danger"
                        onclick={stopDesktopRtmp}
                      >
                        🛑 Стоп
                      </button>
                    {:else}
                      <button
                        type="button"
                        class="btn btn-sm btn-stream-win"
                        disabled={!ffmpegInfo.installed || !isRtmpConfigured || isStartingRtmp}
                        onclick={() => switchStreamWindow(win)}
                        title={rtmpStreamStatus ? 'Переключить трансляцию на это окно игры' : 'Начать трансляцию этого окна'}
                      >
                        {rtmpStreamStatus ? '🔄 Переключить эфир сюда' : '▶️ Стримить это окно'}
                      </button>
                    {/if}
                  </div>
                </div>
              {/each}
            </div>
          {/if}
        </div>
      </div>

      <!-- ================= RTMP CREDENTIALS & SETTINGS ================= -->
      <div class="credentials-card">
        <h3 class="credentials-title">⚙️ Параметры подключения к Telegram</h3>

        <!-- Quality & Resolution Selection -->
        <div class="resolution-selector-group">
          <span class="section-sublabel">Качество видеотрансляции (Разрешение):</span>
          <div class="resolution-options">
            <button
              type="button"
              class="res-card"
              class:active={selectedResolution === '1080p'}
              onclick={() => setResolution('1080p')}
            >
              <div class="res-radio-row">
                <span class="res-title">1080p Full HD</span>
                {#if selectedResolution === '1080p'}
                  <span class="res-check">✅ Выбрано</span>
                {/if}
              </div>
              <div class="res-specs">1920 × 1080 • 30 FPS • Битрейт ~4500k</div>
              <div class="res-desc">
                Максимальная четкость игрового интерфейса, текста и ников. Рекомендуется при стабильном интернете (от 10 Мбит/с отдачи).
              </div>
            </button>

            <button
              type="button"
              class="res-card"
              class:active={selectedResolution === '720p'}
              onclick={() => setResolution('720p')}
            >
              <div class="res-radio-row">
                <span class="res-title">720p HD</span>
                {#if selectedResolution === '720p'}
                  <span class="res-check">✅ Выбрано</span>
                {/if}
              </div>
              <div class="res-specs">1280 × 720 • 30 FPS • Битрейт ~2500k</div>
              <div class="res-desc">
                Экономичный режим. Снижает нагрузку на процессор и интернет. Идеально при медленном или мобильном соединении.
              </div>
            </button>
          </div>
        </div>

        <!-- RTMP Credentials Inputs -->
        <div class="form-grid">
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
            <span class="hint">Скопируйте «URL сервера» из меню канала: «Видеотрансляция» ➔ «Трансляция с помощью...»</span>
          </div>

          <!-- RTMP Key Input -->
          <div class="form-group">
            <div class="form-label-row">
              <label for="rtmp-key">Ключ трансляции Telegram (Stream Key)</label>
              <div class="key-actions">
                <button
                  type="button"
                  class="btn-text-sm"
                  onclick={() => (showRtmpKey = !showRtmpKey)}
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
                if (!config.streamer) config.streamer = { rtmp_url: '', rtmp_key: '', rtmp_channel_url: '' };
                config.streamer.rtmp_key = (e.target as HTMLInputElement).value;
              }}
              placeholder="123456789:ABCdefGHIjklMNO..."
            />
            <span class="hint">Ключ из настроек трансляции канала (сохраняется локально в конфиге программы)</span>
          </div>
        </div>

        <!-- Telegram Channel Link Input -->
        <div class="form-group">
          <div class="form-label-row">
            <label for="rtmp-channel-url">Ссылка на канал или группу Telegram (для бота и быстрого перехода)</label>
            <button
              type="button"
              class="btn-text-sm"
              onclick={pasteChannelUrlFromClipboard}
            >
              📋 Вставить
            </button>
          </div>
          <input
            id="rtmp-channel-url"
            type="text"
            value={config.streamer?.rtmp_channel_url ?? ''}
            oninput={(e) => {
              if (!config.streamer) config.streamer = { rtmp_url: '', rtmp_key: '', rtmp_channel_url: '' };
              config.streamer.rtmp_channel_url = (e.target as HTMLInputElement).value;
            }}
            placeholder="https://t.me/my_channel или @my_channel"
          />
          <span class="hint">Укажите ваш канал — бот и приложение добавят прямую кнопку «📢 Открыть канал» для просмотра</span>
        </div>

        <!-- Action Buttons Row: Save & Test Connection -->
        <div class="actions-row">
          <div class="left-actions">
            <button
              type="button"
              class="btn btn-primary"
              disabled={isSaving}
              onclick={saveSettings}
            >
              {isSaving ? 'Сохранение...' : '💾 Сохранить настройки стрима'}
            </button>
            {#if saveSuccess}
              <span class="save-toast">✅ Сохранено!</span>
            {/if}
          </div>

          <div class="test-actions">
            <button
              type="button"
              class="btn btn-outline"
              disabled={isTestingRtmp || !config.streamer?.rtmp_key?.trim()}
              onclick={testRtmpConnection}
            >
              {isTestingRtmp ? '⏳ Проверка соединения с Telegram...' : '🔍 Проверить подключение к RTMP'}
            </button>
          </div>
        </div>

        {#if rtmpTestResult}
          <div class="test-badge-box" class:success={rtmpTestResult.ok} class:error={!rtmpTestResult.ok}>
            {rtmpTestResult.message}
          </div>
        {/if}
      </div>

      <!-- ================= IMPORTANT TIP: PERSISTENT BROADCAST ================= -->
      <div class="persistent-broadcast-tip">
        <div class="tip-icon-cell">💡</div>
        <div class="tip-body-cell">
          <div class="tip-head">Полезный лайфхак: один раз включили в Telegram — и больше не выключайте!</div>
          <div class="tip-text">
            В Telegram-канале нажмите синюю кнопку <strong>«Начать трансляцию» всего один раз</strong>.
            Когда вы заканчиваете играть или останавливаете вещание в ew-notify, <strong>НЕ нажимайте «Завершить трансляцию» в Telegram</strong>!
          </div>
          <div class="tip-subtext">
            Канал перейдёт в режим ожидания сигнала. При следующем запуске игры просто нажмите <strong>«▶️ Стримить это окно»</strong> в ew-notify или кнопку <strong>📡 RTMP</strong> в Telegram-боте — эфир сразу же возобновится в том же плеере, а постоянный ключ трансляции не сбросится!
          </div>
        </div>
      </div>

      <!-- ================= VISUAL STEP-BY-STEP GUIDE ================= -->
      <div class="stream-guide-card">
        <div class="guide-title">📖 Пошаговый гайд: как запустить стрим в Telegram</div>
        <div class="guide-steps">
          <div class="step-item">
            <div class="step-num">1</div>
            <div class="step-content">
              <strong>Создайте закрытый канал или группу</strong>
              <p>В Telegram создайте частный (private) канал или группу для себя или своей пачки.</p>
            </div>
          </div>
          <div class="step-item">
            <div class="step-num">2</div>
            <div class="step-content">
              <strong>Включите видеотрансляцию в канале</strong>
              <p>В меню канала выберите <strong>«Видеотрансляция»</strong> ➔ <strong>«Трансляция с помощью стороннего ПО»</strong>.</p>
            </div>
          </div>
          <div class="step-item">
            <div class="step-num">3</div>
            <div class="step-content">
              <strong>Вставьте параметры сервера и ключ</strong>
              <p>Скопируйте из окна Telegram «URL сервера» и «Ключ трансляции» и нажмите <strong>«📋 Вставить»</strong> в поля выше, затем нажмите «Сохранить».</p>
            </div>
          </div>
          <div class="step-item">
            <div class="step-num">4</div>
            <div class="step-content">
              <strong>Запустите эфир окна игры</strong>
              <p>В блоке менеджера окон выше нажмите <strong>«▶️ Стримить это окно»</strong> (или отправьте боту команду <code>/rtmp</code>).</p>
            </div>
          </div>
          <div class="step-item highlight">
            <div class="step-num">5</div>
            <div class="step-content">
              <strong>Нажмите «Начать трансляцию» в Telegram (один раз!)</strong>
              <p>В окне Telegram нажмите кнопку <strong>«Начать трансляцию»</strong> — прямой эфир появится на экране! Не нажимайте «Завершить трансляцию», и стрим будет работать всегда.</p>
            </div>
          </div>
        </div>
      </div>
    </div>
  </div>

  <!-- SECTION 2: P2P WebRTC & UPnP (Mini App) -->
  <div class="card p2p-card">
    <div class="card-header">
      <div class="header-left">
        <h2>🌐 P2P WebRTC Стриминг (Telegram Mini App)</h2>
        <span class="info-pill">Прямое подключение</span>
      </div>
      <div class="header-right">
        {#if isCheckingUpnp}
          <span class="badge badge-warning">
            <span class="dot dot-warning"></span> ⏳ Проверка UPnP...
          </span>
        {:else if upnpResult}
          <span class="badge" class:badge-success={upnpResult.available} class:badge-warning={!upnpResult.available}>
            <span class="dot" class:dot-success={upnpResult.available} class:dot-warning={!upnpResult.available}></span>
            UPnP {upnpResult.available ? 'Активен' : 'Недоступен'}
          </span>
        {/if}
      </div>
    </div>

    <div class="card-body">
      <!-- High-visibility Recommendation Banner -->
      <div class="p2p-recommendation-box">
        <div class="rec-icon">💡</div>
        <div class="rec-content">
          <div class="rec-title">Важная рекомендация по выбору технологии трансляции:</div>
          <div class="rec-text">
            P2P WebRTC соединяет ваш телефон и ПК <strong>напрямую без серверов</strong>. Это работает мгновенно с нулевой задержкой, если оба устройства находятся в одной домашней сети (Wi-Fi/LAN) или если домашний роутер поддерживает технологию <strong>UPnP</strong>.
          </div>
          <div class="rec-text strong">
            📱 Если ваш телефон находится вне дома (на мобильном интернете 4G/LTE), а роутер не поддерживает UPnP или находится за провайдерским NAT (CGNAT) — прямое P2P соединение блокируется оператором.
          </div>
          <div class="rec-text solution">
            👉 В таком случае используйте <strong>RTMP-трансляцию в закрытый канал Telegram</strong> (настроена в карточке выше). Видеопоток передается через сервера Telegram, поэтому эфир работает гарантированно на любом расстоянии и с любого смартфона!
          </div>
        </div>
      </div>

      <!-- UPnP Diagnostics & Status Block -->
      <div class="upnp-check-section">
        <div class="upnp-header-row">
          <div>
            <strong>Статус UPnP на вашем домашнем роутере:</strong>
            <div class="hint">UPnP автоматически открывает порт на домашнем роутере для подключения к стриму через интернет. Проверяется автоматически при запуске.</div>
          </div>
          <button
            type="button"
            class="btn btn-outline btn-sm"
            disabled={isCheckingUpnp}
            onclick={checkUpnp}
          >
            {isCheckingUpnp ? '⏳ Опрос роутера...' : '🔄 Проверить заново'}
          </button>
        </div>

        {#if isCheckingUpnp && !upnpResult}
          <div class="upnp-checking-indicator">
            <span class="checking-spinner"></span>
            <span>Выполняется фоновый опрос шлюза по протоколу SSDP/UPnP...</span>
          </div>
        {:else if upnpResult}
          <div class="upnp-result-card" class:upnp-ok={upnpResult.available} class:upnp-fail={!upnpResult.available}>
            <div class="result-status-row">
              <span class="status-icon">{upnpResult.available ? '✅' : '⚠️'}</span>
              <span class="status-summary">
                {upnpResult.available
                  ? 'Роутер поддерживает UPnP и доступен для прямого подключения.'
                  : 'Роутер не ответил на UPnP запрос (или UPnP отключен в настройках).'}
              </span>
            </div>
            {#if upnpResult.external_ip}
              <div class="result-detail-row">
                <span>Внешний IP роутера:</span>
                <code>{upnpResult.external_ip}</code>
              </div>
            {/if}
            <div class="result-msg">{upnpResult.details}</div>
          </div>
        {/if}
      </div>
    </div>
  </div>
</div>

<style>
  .stream-page {
    display: flex;
    flex-direction: column;
    gap: 20px;
  }

  .page-header h1 {
    font-size: 1.5rem;
    font-weight: 700;
    margin: 0 0 6px 0;
    color: #f8fafc;
  }

  .subtitle {
    margin: 0;
    font-size: 0.88rem;
    color: #94a3b8;
  }

  .card {
    background: #1e293b;
    border: 1px solid rgba(255, 255, 255, 0.08);
    border-radius: 12px;
    overflow: hidden;
  }

  .card-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 16px 20px;
    border-bottom: 1px solid rgba(255, 255, 255, 0.06);
    background: rgba(15, 23, 42, 0.4);
  }

  .header-left {
    display: flex;
    align-items: center;
    gap: 12px;
  }

  .header-left h2 {
    font-size: 1.15rem;
    font-weight: 600;
    margin: 0;
    color: #f1f5f9;
  }

  .header-right {
    display: flex;
    align-items: center;
    gap: 8px;
  }

  .recommended-pill {
    font-size: 11px;
    font-weight: 600;
    padding: 3px 8px;
    border-radius: 999px;
    background: rgba(56, 189, 248, 0.15);
    color: #38bdf8;
    border: 1px solid rgba(56, 189, 248, 0.35);
  }

  .info-pill {
    font-size: 11px;
    font-weight: 600;
    padding: 3px 8px;
    border-radius: 999px;
    background: rgba(148, 163, 184, 0.15);
    color: #cbd5e1;
    border: 1px solid rgba(148, 163, 184, 0.3);
  }

  .badge {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    font-size: 11px;
    font-weight: 600;
    padding: 4px 10px;
    border-radius: 999px;
  }

  .badge-success {
    background: rgba(34, 197, 94, 0.15);
    color: #4ade80;
    border: 1px solid rgba(34, 197, 94, 0.35);
  }

  .badge-warning {
    background: rgba(234, 179, 8, 0.15);
    color: #facc15;
    border: 1px solid rgba(234, 179, 8, 0.35);
  }

  .dot {
    width: 6px;
    height: 6px;
    border-radius: 50%;
  }

  .dot-success {
    background: #22c55e;
    box-shadow: 0 0 6px #22c55e;
  }

  .dot-warning {
    background: #eab308;
    box-shadow: 0 0 6px #eab308;
  }

  .btn-text-danger-sm {
    background: none;
    border: 1px solid rgba(239, 68, 68, 0.3);
    color: #f87171;
    padding: 2px 7px;
    border-radius: 4px;
    cursor: pointer;
    font-size: 12px;
  }

  .btn-text-danger-sm:hover {
    background: rgba(239, 68, 68, 0.15);
    border-color: #ef4444;
  }

  .card-body {
    padding: 20px;
    display: flex;
    flex-direction: column;
    gap: 20px;
  }

  /* FFmpeg Alert Box */
  .ffmpeg-alert-box {
    padding: 14px 16px;
    background: rgba(234, 179, 8, 0.08);
    border: 1px solid rgba(234, 179, 8, 0.25);
    border-radius: 8px;
    color: #fef08a;
  }

  .alert-title {
    font-size: 0.95rem;
    margin-bottom: 4px;
    color: #fde047;
  }

  .alert-desc {
    font-size: 0.85rem;
    line-height: 1.45;
    color: #fef9c3;
    margin-bottom: 12px;
  }

  .alert-actions {
    display: flex;
    gap: 10px;
    flex-wrap: wrap;
  }

  .btn-dl-ffmpeg {
    font-weight: 600;
  }

  .dl-progress-box {
    margin-top: 10px;
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
    color: #f87171;
    font-size: 12px;
  }

  .install-msg {
    margin-top: 8px;
    color: #38bdf8;
    font-size: 12px;
  }

  /* ================= STREAM MANAGER CARD ================= */
  .stream-manager-card {
    background: rgba(15, 23, 42, 0.65);
    border: 1px solid rgba(56, 189, 248, 0.25);
    border-radius: 10px;
    padding: 18px;
    display: flex;
    flex-direction: column;
    gap: 16px;
  }

  .manager-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding-bottom: 12px;
    border-bottom: 1px solid rgba(255, 255, 255, 0.08);
  }

  .manager-title-group h3 {
    margin: 0;
    font-size: 1.05rem;
    font-weight: 700;
    color: #f8fafc;
  }

  .manager-sub {
    font-size: 0.8rem;
    color: #94a3b8;
  }

  .manager-header-actions {
    display: flex;
    align-items: center;
    gap: 10px;
  }

  .live-pill {
    display: inline-flex;
    align-items: center;
    gap: 7px;
    background: rgba(239, 68, 68, 0.2);
    border: 1px solid rgba(239, 68, 68, 0.5);
    color: #fca5a5;
    font-size: 11px;
    font-weight: 700;
    padding: 4px 10px;
    border-radius: 999px;
  }

  .offline-pill {
    font-size: 11px;
    font-weight: 600;
    color: #94a3b8;
    background: rgba(255, 255, 255, 0.06);
    border: 1px solid rgba(255, 255, 255, 0.1);
    padding: 3px 10px;
    border-radius: 999px;
  }

  .live-dot-pulse {
    width: 7px;
    height: 7px;
    border-radius: 50%;
    background: #ef4444;
    box-shadow: 0 0 8px #ef4444;
    animation: live-pulse-anim 1.5s infinite;
  }

  @keyframes live-pulse-anim {
    0%, 100% { opacity: 1; transform: scale(1); }
    50% { opacity: 0.35; transform: scale(0.8); }
  }

  .btn-refresh-icon {
    background: rgba(255, 255, 255, 0.08);
    border: 1px solid rgba(255, 255, 255, 0.15);
    border-radius: 6px;
    padding: 5px 9px;
    cursor: pointer;
    font-size: 13px;
    color: #cbd5e1;
    transition: all 0.15s ease;
  }

  .btn-refresh-icon:hover {
    background: rgba(255, 255, 255, 0.18);
  }

  .active-session-banner {
    display: flex;
    justify-content: space-between;
    align-items: center;
    background: rgba(239, 68, 68, 0.08);
    border: 1px solid rgba(239, 68, 68, 0.3);
    border-radius: 8px;
    padding: 14px 16px;
    gap: 16px;
    flex-wrap: wrap;
  }

  .session-main {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }

  .session-game-title {
    display: flex;
    align-items: center;
    gap: 8px;
    font-size: 1.05rem;
    color: #f8fafc;
  }

  .game-tag {
    font-size: 10px;
    font-weight: 700;
    background: #ef4444;
    color: #fff;
    padding: 1px 6px;
    border-radius: 4px;
  }

  .pid-tag {
    font-size: 11px;
    color: #94a3b8;
    background: rgba(255, 255, 255, 0.08);
    padding: 2px 6px;
    border-radius: 4px;
  }

  .session-specs-row {
    display: flex;
    gap: 14px;
    font-size: 0.82rem;
    color: #cbd5e1;
    flex-wrap: wrap;
  }

  .session-specs-row strong {
    color: #f1f5f9;
  }

  .session-action-btns {
    display: flex;
    gap: 8px;
    align-items: center;
    flex-wrap: wrap;
  }

  .btn-outline-channel {
    background: rgba(56, 189, 248, 0.12);
    border: 1px solid rgba(56, 189, 248, 0.4);
    color: #38bdf8;
    font-weight: 600;
  }

  .btn-outline-channel:hover {
    background: rgba(56, 189, 248, 0.25);
  }

  .btn-restart {
    background: rgba(234, 179, 8, 0.15);
    border: 1px solid rgba(234, 179, 8, 0.4);
    color: #facc15;
    font-weight: 600;
  }

  .btn-restart:hover:not(:disabled) {
    background: rgba(234, 179, 8, 0.25);
  }

  .preflight-bar {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(200px, 1fr));
    gap: 10px;
  }

  .preflight-item {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 8px 12px;
    background: rgba(15, 23, 42, 0.5);
    border: 1px solid rgba(255, 255, 255, 0.08);
    border-radius: 6px;
    font-size: 0.82rem;
    color: #94a3b8;
  }

  .preflight-item.ok {
    border-color: rgba(34, 197, 94, 0.25);
    color: #cbd5e1;
  }

  .preflight-item.ok strong {
    color: #4ade80;
  }

  /* Game Windows List */
  .game-windows-section {
    display: flex;
    flex-direction: column;
    gap: 10px;
  }

  .windows-section-title {
    display: flex;
    justify-content: space-between;
    font-size: 0.85rem;
    font-weight: 600;
    color: #cbd5e1;
  }

  .windows-count {
    color: #64748b;
    font-size: 0.8rem;
  }

  .empty-windows-box {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 10px;
    padding: 24px;
    background: rgba(15, 23, 42, 0.4);
    border: 1px dashed rgba(255, 255, 255, 0.12);
    border-radius: 8px;
    text-align: center;
  }

  .empty-icon {
    font-size: 28px;
    opacity: 0.6;
  }

  .empty-text {
    font-size: 0.85rem;
    color: #94a3b8;
    max-width: 440px;
  }

  .windows-list {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  .window-item-card {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 10px 14px;
    background: rgba(15, 23, 42, 0.55);
    border: 1px solid rgba(255, 255, 255, 0.08);
    border-radius: 8px;
    transition: all 0.15s ease;
  }

  .window-item-card:hover {
    background: rgba(15, 23, 42, 0.75);
    border-color: rgba(56, 189, 248, 0.3);
  }

  .window-item-card.is-streaming {
    background: rgba(239, 68, 68, 0.08);
    border-color: rgba(239, 68, 68, 0.4);
  }

  .win-left-info {
    display: flex;
    align-items: center;
    gap: 10px;
  }

  .slot-badge {
    font-size: 11px;
    font-weight: 700;
    color: #38bdf8;
    background: rgba(56, 189, 248, 0.15);
    border: 1px solid rgba(56, 189, 248, 0.3);
    padding: 2px 7px;
    border-radius: 4px;
  }

  .win-meta {
    display: flex;
    flex-direction: column;
    gap: 2px;
  }

  .win-title-row {
    display: flex;
    align-items: center;
    gap: 8px;
  }

  .win-title {
    font-size: 0.92rem;
    color: #f1f5f9;
  }

  .minimized-tag {
    font-size: 10px;
    color: #eab308;
    background: rgba(234, 179, 8, 0.15);
    padding: 1px 5px;
    border-radius: 3px;
  }

  .win-pid-info {
    font-size: 0.76rem;
    color: #64748b;
  }

  .win-right-actions {
    display: flex;
    align-items: center;
    gap: 8px;
  }

  .current-live-tag {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    font-size: 11px;
    font-weight: 700;
    color: #f87171;
    background: rgba(239, 68, 68, 0.15);
    padding: 4px 8px;
    border-radius: 4px;
  }

  .btn-stream-win {
    background: #059669;
    color: #fff;
    font-weight: 600;
    transition: all 0.15s ease;
  }

  .btn-stream-win:hover:not(:disabled) {
    background: #10b981;
  }

  .btn-stream-win:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }

  .rtmp-error-msg {
    padding: 8px 12px;
    background: rgba(239, 68, 68, 0.15);
    border: 1px solid rgba(239, 68, 68, 0.35);
    border-radius: 6px;
    color: #fca5a5;
    font-size: 0.85rem;
  }

  /* ================= CREDENTIALS CARD ================= */
  .credentials-card {
    background: rgba(15, 23, 42, 0.4);
    border: 1px solid rgba(255, 255, 255, 0.08);
    border-radius: 10px;
    padding: 18px;
    display: flex;
    flex-direction: column;
    gap: 16px;
  }

  .credentials-title {
    margin: 0;
    font-size: 1rem;
    font-weight: 600;
    color: #f1f5f9;
  }

  .resolution-selector-group {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  .section-sublabel {
    font-size: 0.85rem;
    font-weight: 600;
    color: #cbd5e1;
  }

  .resolution-options {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(280px, 1fr));
    gap: 12px;
  }

  .res-card {
    display: flex;
    flex-direction: column;
    gap: 6px;
    padding: 12px 14px;
    background: rgba(15, 23, 42, 0.6);
    border: 1px solid rgba(255, 255, 255, 0.1);
    border-radius: 8px;
    text-align: left;
    cursor: pointer;
    transition: all 0.15s ease;
    color: #cbd5e1;
  }

  .res-card:hover {
    background: rgba(15, 23, 42, 0.8);
    border-color: rgba(56, 189, 248, 0.4);
  }

  .res-card.active {
    background: rgba(56, 189, 248, 0.08);
    border-color: #38bdf8;
    box-shadow: 0 0 12px rgba(56, 189, 248, 0.15);
  }

  .res-radio-row {
    display: flex;
    justify-content: space-between;
    align-items: center;
  }

  .res-title {
    font-size: 0.95rem;
    font-weight: 700;
    color: #f8fafc;
  }

  .res-check {
    font-size: 11px;
    font-weight: 600;
    color: #38bdf8;
  }

  .res-specs {
    font-size: 0.8rem;
    font-weight: 600;
    color: #4ade80;
  }

  .res-desc {
    font-size: 0.76rem;
    line-height: 1.4;
    color: #94a3b8;
  }

  .form-grid {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 14px;
  }

  @media (max-width: 768px) {
    .form-grid {
      grid-template-columns: 1fr;
    }
  }

  .form-group {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }

  .form-label-row {
    display: flex;
    justify-content: space-between;
    align-items: center;
  }

  .form-label-row label {
    font-size: 0.84rem;
    font-weight: 600;
    color: #cbd5e1;
  }

  .key-actions {
    display: flex;
    gap: 6px;
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

  input[type='text'],
  input[type='password'] {
    background: #0f172a;
    border: 1px solid rgba(255, 255, 255, 0.15);
    border-radius: 6px;
    padding: 8px 12px;
    color: #f8fafc;
    font-size: 0.88rem;
    transition: border-color 0.15s ease;
  }

  input[type='text']:focus,
  input[type='password']:focus {
    outline: none;
    border-color: #38bdf8;
  }

  .hint {
    font-size: 0.74rem;
    color: #64748b;
  }

  .actions-row {
    display: flex;
    justify-content: space-between;
    align-items: center;
    gap: 12px;
    flex-wrap: wrap;
    padding-top: 4px;
  }

  .left-actions {
    display: flex;
    align-items: center;
    gap: 12px;
  }

  .save-toast {
    font-size: 0.85rem;
    font-weight: 600;
    color: #4ade80;
  }

  .test-badge-box {
    padding: 8px 12px;
    border-radius: 6px;
    font-size: 0.84rem;
  }

  .test-badge-box.success {
    background: rgba(34, 197, 94, 0.15);
    border: 1px solid rgba(34, 197, 94, 0.35);
    color: #4ade80;
  }

  .test-badge-box.error {
    background: rgba(239, 68, 68, 0.15);
    border: 1px solid rgba(239, 68, 68, 0.35);
    color: #fca5a5;
  }

  /* ================= PERSISTENT BROADCAST TIP ================= */
  .persistent-broadcast-tip {
    display: flex;
    gap: 14px;
    padding: 14px 16px;
    background: linear-gradient(135deg, rgba(56, 189, 248, 0.1) 0%, rgba(99, 102, 241, 0.08) 100%);
    border: 1px solid rgba(56, 189, 248, 0.35);
    border-radius: 10px;
  }

  .tip-icon-cell {
    font-size: 24px;
    flex-shrink: 0;
  }

  .tip-body-cell {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }

  .tip-head {
    font-size: 0.94rem;
    font-weight: 700;
    color: #38bdf8;
  }

  .tip-text {
    font-size: 0.85rem;
    line-height: 1.45;
    color: #e2e8f0;
  }

  .tip-subtext {
    font-size: 0.8rem;
    line-height: 1.4;
    color: #94a3b8;
  }

  /* ================= STEP-BY-STEP GUIDE ================= */
  .stream-guide-card {
    background: rgba(15, 23, 42, 0.4);
    border: 1px solid rgba(255, 255, 255, 0.08);
    border-radius: 10px;
    padding: 18px;
    display: flex;
    flex-direction: column;
    gap: 14px;
  }

  .guide-title {
    font-size: 0.95rem;
    font-weight: 700;
    color: #38bdf8;
  }

  .guide-steps {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(200px, 1fr));
    gap: 12px;
  }

  .step-item {
    display: flex;
    flex-direction: column;
    gap: 8px;
    padding: 12px;
    background: rgba(15, 23, 42, 0.6);
    border: 1px solid rgba(255, 255, 255, 0.06);
    border-radius: 8px;
  }

  .step-item.highlight {
    background: rgba(56, 189, 248, 0.08);
    border-color: rgba(56, 189, 248, 0.3);
  }

  .step-num {
    width: 24px;
    height: 24px;
    border-radius: 50%;
    background: #0284c7;
    color: #fff;
    display: flex;
    align-items: center;
    justify-content: center;
    font-size: 11px;
    font-weight: 700;
  }

  .step-item.highlight .step-num {
    background: #38bdf8;
    color: #0f172a;
  }

  .step-content strong {
    font-size: 0.85rem;
    color: #f1f5f9;
  }

  .step-content p {
    margin: 4px 0 0 0;
    font-size: 0.77rem;
    line-height: 1.4;
    color: #94a3b8;
  }

  /* ================= P2P SECTION ================= */
  .p2p-recommendation-box {
    display: flex;
    gap: 14px;
    padding: 14px 16px;
    background: rgba(234, 179, 8, 0.08);
    border: 1px solid rgba(234, 179, 8, 0.3);
    border-radius: 8px;
  }

  .rec-icon {
    font-size: 24px;
    flex-shrink: 0;
  }

  .rec-content {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }

  .rec-title {
    font-size: 0.94rem;
    font-weight: 700;
    color: #facc15;
  }

  .rec-text {
    font-size: 0.83rem;
    line-height: 1.45;
    color: #fef08a;
  }

  .rec-text.strong {
    color: #fed7aa;
    font-weight: 500;
  }

  .rec-text.solution {
    color: #bae6fd;
    font-weight: 600;
  }

  .upnp-check-section {
    display: flex;
    flex-direction: column;
    gap: 12px;
    padding: 14px;
    background: rgba(15, 23, 42, 0.4);
    border: 1px solid rgba(255, 255, 255, 0.08);
    border-radius: 8px;
  }

  .upnp-header-row {
    display: flex;
    justify-content: space-between;
    align-items: center;
    gap: 12px;
  }

  .upnp-checking-indicator {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 10px 14px;
    background: rgba(56, 189, 248, 0.08);
    border: 1px solid rgba(56, 189, 248, 0.2);
    border-radius: 6px;
    font-size: 0.82rem;
    color: #38bdf8;
  }

  .checking-spinner {
    width: 14px;
    height: 14px;
    border: 2px solid rgba(56, 189, 248, 0.3);
    border-top-color: #38bdf8;
    border-radius: 50%;
    animation: spin 0.8s linear infinite;
  }

  @keyframes spin {
    to { transform: rotate(360deg); }
  }

  .upnp-result-card {
    padding: 12px;
    border-radius: 6px;
    display: flex;
    flex-direction: column;
    gap: 6px;
    font-size: 0.83rem;
  }

  .upnp-result-card.upnp-ok {
    background: rgba(34, 197, 94, 0.12);
    border: 1px solid rgba(34, 197, 94, 0.35);
    color: #dcfce7;
  }

  .upnp-result-card.upnp-fail {
    background: rgba(234, 179, 8, 0.1);
    border: 1px solid rgba(234, 179, 8, 0.3);
    color: #fef08a;
  }

  .result-status-row {
    display: flex;
    align-items: center;
    gap: 8px;
    font-weight: 600;
  }

  .result-detail-row {
    display: flex;
    gap: 8px;
    align-items: center;
  }

  .result-detail-row code {
    background: rgba(0, 0, 0, 0.3);
    padding: 2px 6px;
    border-radius: 4px;
    color: #38bdf8;
    font-family: monospace;
  }

  .result-msg {
    font-size: 0.77rem;
    opacity: 0.85;
  }

  /* Common Buttons */
  .btn {
    padding: 8px 16px;
    border-radius: 6px;
    font-size: 0.85rem;
    font-weight: 500;
    cursor: pointer;
    border: none;
    transition: all 0.15s ease;
  }

  .btn-sm {
    padding: 5px 12px;
    font-size: 0.78rem;
  }

  .btn-primary {
    background: #0284c7;
    color: #fff;
  }

  .btn-primary:hover:not(:disabled) {
    background: #0369a1;
  }

  .btn-danger {
    background: #dc2626;
    color: #fff;
  }

  .btn-danger:hover:not(:disabled) {
    background: #b91c1c;
  }

  .btn-outline {
    background: transparent;
    border: 1px solid rgba(255, 255, 255, 0.18);
    color: #cbd5e1;
  }

  .btn-outline:hover:not(:disabled) {
    background: rgba(255, 255, 255, 0.08);
    border-color: rgba(255, 255, 255, 0.3);
  }

  .btn:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }
</style>
