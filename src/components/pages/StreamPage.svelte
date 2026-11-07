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

  let unlistenFfmpeg: UnlistenFn | null = null;

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

    try {
      unlistenFfmpeg = await listen<FFmpegDownloadProgress>('ffmpeg-download-progress', (event) => {
        ffmpegDownload = event.payload;
        if (event.payload.stage === 'ready') {
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
    if (unlistenFfmpeg) unlistenFfmpeg();
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

  async function startDesktopRtmp() {
    isStartingRtmp = true;
    rtmpActionError = null;
    try {
      const key = (config.streamer?.rtmp_key ?? '').trim();
      if (!key) {
        rtmpActionError = 'Ключ трансляции не указан в настройках выше.';
        return;
      }
      if (!selectedHwnd) {
        rtmpActionError = 'Выберите окно игры для трансляции.';
        return;
      }
      const win = targetWindows.find((w) => w.hwnd === selectedHwnd);
      if (!win) {
        rtmpActionError = 'Выбранное окно больше не найдено. Нажмите 🔄 Обновить.';
        return;
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
    upnpResult = null;
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

  <!-- Live Status Banner if active -->
  {#if rtmpStreamStatus}
    <div class="live-banner">
      <div class="live-badge">
        <span class="live-pulse"></span>
        <span class="live-text">В ЭФИРЕ</span>
      </div>
      <div class="live-info">
        <div class="live-title">🎮 {rtmpStreamStatus.title} (PID {rtmpStreamStatus.pid})</div>
        <div class="live-meta">
          <span>🚀 Разрешение: <strong>{rtmpStreamStatus.resolution || '1080p Full HD'} (30 FPS)</strong></span>
          <span>⏱ В эфире: <strong>{formatUptime(rtmpStreamStatus.uptime_secs)}</strong></span>
        </div>
      </div>
      <div class="live-actions">
        {#if config.streamer?.rtmp_channel_url}
          <button
            type="button"
            class="btn btn-sm btn-outline-white"
            onclick={() => api.openUrl(config.streamer?.rtmp_channel_url || '')}
          >
            📢 Открыть канал
          </button>
        {/if}
        <button
          type="button"
          class="btn btn-sm btn-danger"
          onclick={stopDesktopRtmp}
        >
          🛑 Остановить вещание
        </button>
      </div>
    </div>
  {/if}

  <!-- SECTION 1: RTMP Telegram Streaming -->
  <div class="card stream-card">
    <div class="card-header">
      <div class="header-left">
        <h2>📡 Прямая видеотрансляция в Telegram (RTMP)</h2>
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
              Экономичный режим. Снижает нагрузку на процессор и интернет. Идеально при медленном или нестабильном соединении.
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
          <span class="hint">Ключ из настроек трансляции канала (сохраняется на вашем компьютере)</span>
        </div>
      </div>

      <!-- Telegram Channel Link Input -->
      <div class="form-group">
        <div class="form-label-row">
          <label for="rtmp-channel-url">Ссылка на канал или группу Telegram (для бота)</label>
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
        <span class="hint">Укажите ваш канал — бот пришлёт удобную кнопку «📢 Открыть канал со стримом» для быстрого перехода</span>
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

      <!-- Launch Live Stream Panel -->
      <div class="rtmp-launch-panel">
        <div class="panel-header">
          <span class="panel-title">Управление прямым эфиром прямо с ПК:</span>
        </div>

        {#if rtmpActionError}
          <div class="rtmp-error-msg">⚠️ {rtmpActionError}</div>
        {/if}

        <div class="panel-body">
          {#if rtmpStreamStatus}
            <div class="active-stream-box">
              <div class="box-text">
                Сейчас идёт трансляция: <strong>{rtmpStreamStatus.title}</strong> (PID {rtmpStreamStatus.pid}) | 🚀 {rtmpStreamStatus.resolution} (30 FPS)
              </div>
              <button
                type="button"
                class="btn btn-danger btn-sm"
                onclick={stopDesktopRtmp}
              >
                🛑 Остановить вещание
              </button>
            </div>
          {:else}
            <div class="launch-controls">
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
                class="btn btn-success"
                disabled={!selectedHwnd || !ffmpegInfo.installed || !isRtmpConfigured || isStartingRtmp}
                onclick={startDesktopRtmp}
              >
                {isStartingRtmp ? 'Запуск...' : '▶️ Начать эфир в Telegram'}
              </button>
            </div>
            {#if !isRtmpConfigured}
              <div class="config-warn-box">
                ⚠️ Чтобы активировать запуск, заполните <strong>«Сервер трансляции»</strong> и <strong>«Ключ трансляции»</strong> выше.
              </div>
            {/if}
          {/if}
        </div>
      </div>

      <!-- Visual Step-by-Step Guide -->
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
              <strong>Запустите эфир</strong>
              <p>Нажмите <strong>«▶️ Начать эфир в Telegram»</strong> выше (или отправьте боту команду <code>/rtmp</code>).</p>
            </div>
          </div>
          <div class="step-item highlight">
            <div class="step-num">5</div>
            <div class="step-content">
              <strong>Нажмите «Начать трансляцию» в Telegram</strong>
              <p>В окне Telegram нажмите синюю кнопку <strong>«Начать трансляцию»</strong> — прямой эфир появится на экране!</p>
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
        {#if upnpResult}
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
            📱 Если ваш телефон находится вне дома (на мобильном интернете 4G/LTE), а роутер не поддерживает UPnP или находится за провайдерским NAT (CGNAT) — P2P соединение может блокироваться оператором.
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
            <strong>Статус UPnP на вашем роутере:</strong>
            <div class="hint">UPnP автоматически открывает порт на домашнем роутере для подключения к стриму через интернет.</div>
          </div>
          <button
            type="button"
            class="btn btn-outline btn-sm"
            disabled={isCheckingUpnp}
            onclick={checkUpnp}
          >
            {isCheckingUpnp ? '⏳ Опрос роутера...' : '🔍 Проверить UPnP на роутере'}
          </button>
        </div>

        {#if upnpResult}
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
    max-width: 1000px;
    margin: 0 auto;
    padding-bottom: 40px;
  }

  .page-header {
    margin-bottom: 4px;
  }

  .page-header h1 {
    font-size: 1.5rem;
    font-weight: 700;
    margin: 0 0 6px 0;
    color: var(--text, #fff);
  }

  .subtitle {
    margin: 0;
    font-size: 0.9rem;
    color: var(--text-muted, #94a3b8);
    line-height: 1.4;
  }

  /* Live Banner */
  .live-banner {
    display: flex;
    align-items: center;
    gap: 16px;
    padding: 14px 18px;
    background: linear-gradient(135deg, rgba(239, 68, 68, 0.18) 0%, rgba(220, 38, 38, 0.28) 100%);
    border: 1px solid rgba(239, 68, 68, 0.5);
    border-radius: 10px;
    box-shadow: 0 4px 16px rgba(239, 68, 68, 0.2);
    flex-wrap: wrap;
  }

  .live-badge {
    display: inline-flex;
    align-items: center;
    gap: 8px;
    background: #ef4444;
    color: #fff;
    padding: 4px 10px;
    border-radius: 6px;
    font-size: 0.75rem;
    font-weight: 800;
    letter-spacing: 0.05em;
  }

  .live-pulse {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    background: #fff;
    animation: livePulse 1.2s infinite ease-in-out;
  }

  @keyframes livePulse {
    0%, 100% { opacity: 1; transform: scale(1); }
    50% { opacity: 0.3; transform: scale(0.8); }
  }

  .live-info {
    flex: 1;
    min-width: 240px;
  }

  .live-title {
    font-size: 0.95rem;
    font-weight: 700;
    color: #fff;
    margin-bottom: 2px;
  }

  .live-meta {
    display: flex;
    gap: 16px;
    font-size: 0.82rem;
    color: rgba(255, 255, 255, 0.85);
  }

  .live-actions {
    display: flex;
    gap: 10px;
    align-items: center;
  }

  .btn-outline-white {
    background: rgba(255, 255, 255, 0.15);
    border: 1px solid rgba(255, 255, 255, 0.35);
    color: #fff;
  }

  .btn-outline-white:hover {
    background: rgba(255, 255, 255, 0.25);
  }

  /* Cards */
  .card {
    background: var(--surface, #1e293b);
    border: 1px solid var(--border, #334155);
    border-radius: 10px;
    overflow: hidden;
  }

  .card-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 14px 20px;
    border-bottom: 1px solid var(--border, #334155);
    background: rgba(0, 0, 0, 0.1);
    flex-wrap: wrap;
    gap: 10px;
  }

  .header-left {
    display: flex;
    align-items: center;
    gap: 10px;
    flex-wrap: wrap;
  }

  .header-left h2 {
    font-size: 1.05rem;
    font-weight: 700;
    margin: 0;
    color: var(--text, #fff);
  }

  .header-right {
    display: flex;
    align-items: center;
    gap: 8px;
  }

  .recommended-pill {
    background: rgba(34, 197, 94, 0.15);
    border: 1px solid rgba(34, 197, 94, 0.4);
    color: #4ade80;
    font-size: 0.72rem;
    font-weight: 700;
    padding: 2px 8px;
    border-radius: 12px;
  }

  .info-pill {
    background: rgba(56, 189, 248, 0.15);
    border: 1px solid rgba(56, 189, 248, 0.4);
    color: #38bdf8;
    font-size: 0.72rem;
    font-weight: 700;
    padding: 2px 8px;
    border-radius: 12px;
  }

  .card-body {
    padding: 20px;
    display: flex;
    flex-direction: column;
    gap: 18px;
  }

  /* Badges */
  .badge {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    font-size: 0.75rem;
    font-weight: 600;
    padding: 3px 8px;
    border-radius: 6px;
  }

  .badge-success {
    background: rgba(34, 197, 94, 0.15);
    border: 1px solid rgba(34, 197, 94, 0.35);
    color: #4ade80;
  }

  .badge-warning {
    background: rgba(245, 158, 11, 0.15);
    border: 1px solid rgba(245, 158, 11, 0.35);
    color: #fbbf24;
  }

  .dot {
    width: 6px;
    height: 6px;
    border-radius: 50%;
  }

  .dot-success { background: #4ade80; }
  .dot-warning { background: #fbbf24; }

  .btn-text-danger-sm {
    background: transparent;
    border: none;
    color: #f87171;
    font-size: 0.9rem;
    cursor: pointer;
    padding: 2px 6px;
    border-radius: 4px;
  }

  .btn-text-danger-sm:hover {
    background: rgba(239, 68, 68, 0.15);
  }

  /* FFmpeg Alert Box */
  .ffmpeg-alert-box {
    background: rgba(245, 158, 11, 0.08);
    border: 1px solid rgba(245, 158, 11, 0.3);
    border-radius: 8px;
    padding: 16px;
  }

  .alert-title {
    font-size: 0.95rem;
    color: #f59e0b;
    margin-bottom: 6px;
  }

  .alert-desc {
    font-size: 0.85rem;
    color: var(--text-muted, #94a3b8);
    line-height: 1.45;
    margin-bottom: 12px;
  }

  .alert-actions {
    display: flex;
    gap: 10px;
    flex-wrap: wrap;
  }

  .btn-dl-ffmpeg {
    background: #f59e0b;
    color: #000;
    font-weight: 700;
    border: none;
  }

  .btn-dl-ffmpeg:hover:not(:disabled) {
    background: #d97706;
  }

  .dl-progress-box {
    margin-top: 10px;
  }

  .dl-progress-header {
    display: flex;
    justify-content: space-between;
    font-size: 0.82rem;
    margin-bottom: 6px;
  }

  .dl-progress-track {
    height: 8px;
    background: rgba(255, 255, 255, 0.1);
    border-radius: 4px;
    overflow: hidden;
  }

  .dl-progress-bar {
    height: 100%;
    background: linear-gradient(90deg, #f59e0b, #10b981);
    transition: width 0.2s ease;
  }

  .dl-error-msg {
    color: #f87171;
    font-size: 0.85rem;
    margin-bottom: 8px;
  }

  .install-msg {
    color: #38bdf8;
    font-size: 0.85rem;
    margin-bottom: 8px;
  }

  /* Resolution Selector */
  .resolution-selector-group {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  .section-sublabel {
    font-size: 0.88rem;
    font-weight: 600;
    color: var(--text, #fff);
  }

  .resolution-options {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(280px, 1fr));
    gap: 12px;
  }

  .res-card {
    text-align: left;
    background: rgba(255, 255, 255, 0.03);
    border: 1px solid var(--border, #334155);
    border-radius: 8px;
    padding: 14px;
    cursor: pointer;
    transition: all 0.2s ease;
  }

  .res-card:hover {
    background: rgba(255, 255, 255, 0.06);
    border-color: rgba(99, 102, 241, 0.5);
  }

  .res-card.active {
    background: rgba(99, 102, 241, 0.12);
    border-color: #6366f1;
    box-shadow: 0 0 0 1px #6366f1;
  }

  .res-radio-row {
    display: flex;
    justify-content: space-between;
    align-items: center;
    margin-bottom: 4px;
  }

  .res-title {
    font-size: 0.95rem;
    font-weight: 700;
    color: #fff;
  }

  .res-check {
    font-size: 0.75rem;
    font-weight: 700;
    color: #818cf8;
  }

  .res-specs {
    font-size: 0.78rem;
    font-weight: 600;
    color: #38bdf8;
    margin-bottom: 6px;
  }

  .res-desc {
    font-size: 0.8rem;
    color: var(--text-muted, #94a3b8);
    line-height: 1.35;
  }

  /* Form Grid */
  .form-grid {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 16px;
  }

  @media (max-width: 720px) {
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

  label {
    font-size: 0.85rem;
    font-weight: 600;
    color: var(--text, #fff);
  }

  input[type="text"],
  input[type="password"] {
    background: rgba(0, 0, 0, 0.25);
    border: 1px solid var(--border, #334155);
    border-radius: 6px;
    padding: 8px 12px;
    color: #fff;
    font-size: 0.88rem;
    outline: none;
    transition: border-color 0.15s ease;
  }

  input[type="text"]:focus,
  input[type="password"]:focus {
    border-color: #6366f1;
  }

  .hint {
    font-size: 0.76rem;
    color: var(--text-muted, #94a3b8);
  }

  .btn-text-sm {
    background: transparent;
    border: none;
    color: #38bdf8;
    font-size: 0.78rem;
    font-weight: 600;
    cursor: pointer;
    padding: 2px 6px;
    border-radius: 4px;
  }

  .btn-text-sm:hover {
    background: rgba(56, 189, 248, 0.15);
  }

  .key-actions {
    display: flex;
    gap: 6px;
  }

  /* Actions Row */
  .actions-row {
    display: flex;
    justify-content: space-between;
    align-items: center;
    flex-wrap: wrap;
    gap: 12px;
    padding-top: 6px;
  }

  .left-actions {
    display: flex;
    align-items: center;
    gap: 12px;
  }

  .save-toast {
    font-size: 0.82rem;
    font-weight: 600;
    color: #4ade80;
  }

  .test-badge-box {
    padding: 8px 14px;
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
    color: #f87171;
  }

  /* Launch Panel */
  .rtmp-launch-panel {
    background: rgba(0, 0, 0, 0.2);
    border: 1px solid var(--border, #334155);
    border-radius: 8px;
    padding: 14px 16px;
  }

  .panel-title {
    font-size: 0.85rem;
    font-weight: 700;
    color: var(--text, #fff);
  }

  .panel-body {
    margin-top: 10px;
  }

  .launch-controls {
    display: flex;
    gap: 10px;
    align-items: center;
    flex-wrap: wrap;
  }

  .window-select {
    flex: 1;
    min-width: 260px;
    background: rgba(0, 0, 0, 0.3);
    border: 1px solid var(--border, #334155);
    border-radius: 6px;
    padding: 7px 10px;
    color: #fff;
    font-size: 0.85rem;
    outline: none;
  }

  .btn-refresh-sm {
    background: rgba(255, 255, 255, 0.08);
    border: 1px solid var(--border, #334155);
    color: #fff;
    padding: 6px 10px;
    border-radius: 6px;
    cursor: pointer;
  }

  .btn-refresh-sm:hover {
    background: rgba(255, 255, 255, 0.15);
  }

  .config-warn-box {
    margin-top: 10px;
    font-size: 0.82rem;
    color: #fbbf24;
    background: rgba(245, 158, 11, 0.1);
    padding: 6px 10px;
    border-radius: 6px;
    border: 1px solid rgba(245, 158, 11, 0.25);
  }

  .active-stream-box {
    display: flex;
    justify-content: space-between;
    align-items: center;
    gap: 12px;
    flex-wrap: wrap;
  }

  .box-text {
    font-size: 0.88rem;
    color: #fff;
  }

  /* Guide Card */
  .stream-guide-card {
    background: rgba(255, 255, 255, 0.02);
    border: 1px solid var(--border, #334155);
    border-radius: 8px;
    padding: 16px;
  }

  .guide-title {
    font-size: 0.92rem;
    font-weight: 700;
    color: #fff;
    margin-bottom: 14px;
  }

  .guide-steps {
    display: flex;
    flex-direction: column;
    gap: 10px;
  }

  .step-item {
    display: flex;
    gap: 12px;
    align-items: flex-start;
  }

  .step-num {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 24px;
    height: 24px;
    border-radius: 50%;
    background: rgba(255, 255, 255, 0.1);
    color: #fff;
    font-size: 0.78rem;
    font-weight: 700;
    flex-shrink: 0;
  }

  .step-item.highlight .step-num {
    background: #6366f1;
    box-shadow: 0 0 10px rgba(99, 102, 241, 0.6);
  }

  .step-content strong {
    font-size: 0.86rem;
    color: #fff;
  }

  .step-content p {
    font-size: 0.8rem;
    color: var(--text-muted, #94a3b8);
    margin: 2px 0 0 0;
    line-height: 1.35;
  }

  /* P2P Recommendation Box */
  .p2p-recommendation-box {
    display: flex;
    gap: 14px;
    background: linear-gradient(135deg, rgba(37, 99, 235, 0.12) 0%, rgba(99, 102, 241, 0.15) 100%);
    border: 1px solid rgba(99, 102, 241, 0.35);
    border-radius: 8px;
    padding: 16px;
  }

  .rec-icon {
    font-size: 1.5rem;
    flex-shrink: 0;
  }

  .rec-content {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }

  .rec-title {
    font-size: 0.95rem;
    font-weight: 700;
    color: #fff;
  }

  .rec-text {
    font-size: 0.83rem;
    color: rgba(255, 255, 255, 0.85);
    line-height: 1.4;
  }

  .rec-text.strong {
    color: #fbbf24;
    font-weight: 500;
  }

  .rec-text.solution {
    color: #4ade80;
    font-weight: 600;
    margin-top: 2px;
  }

  /* UPnP Check Section */
  .upnp-check-section {
    display: flex;
    flex-direction: column;
    gap: 12px;
  }

  .upnp-header-row {
    display: flex;
    justify-content: space-between;
    align-items: center;
    gap: 12px;
    flex-wrap: wrap;
  }

  .upnp-result-card {
    background: rgba(0, 0, 0, 0.2);
    border: 1px solid var(--border, #334155);
    border-radius: 8px;
    padding: 12px 16px;
    display: flex;
    flex-direction: column;
    gap: 6px;
  }

  .upnp-result-card.upnp-ok {
    border-color: rgba(34, 197, 94, 0.4);
  }

  .upnp-result-card.upnp-fail {
    border-color: rgba(245, 158, 11, 0.4);
  }

  .result-status-row {
    display: flex;
    align-items: center;
    gap: 8px;
    font-size: 0.88rem;
    font-weight: 700;
    color: #fff;
  }

  .result-detail-row {
    font-size: 0.8rem;
    color: var(--text-muted, #94a3b8);
    display: flex;
    gap: 8px;
  }

  .result-detail-row code {
    color: #38bdf8;
  }

  .result-msg {
    font-size: 0.8rem;
    color: var(--text-muted, #94a3b8);
    line-height: 1.35;
  }

  /* Common Buttons */
  .btn {
    padding: 8px 14px;
    border-radius: 6px;
    font-size: 0.84rem;
    font-weight: 600;
    cursor: pointer;
    border: none;
    transition: all 0.15s ease;
  }

  .btn-sm {
    padding: 6px 10px;
    font-size: 0.8rem;
  }

  .btn-primary {
    background: #6366f1;
    color: #fff;
  }

  .btn-primary:hover:not(:disabled) {
    background: #4f46e5;
  }

  .btn-success {
    background: #16a34a;
    color: #fff;
  }

  .btn-success:hover:not(:disabled) {
    background: #15803d;
  }

  .btn-danger {
    background: #dc2626;
    color: #fff;
  }

  .btn-danger:hover:not(:disabled) {
    background: #b91c1c;
  }

  .btn-outline {
    background: rgba(255, 255, 255, 0.06);
    border: 1px solid var(--border, #334155);
    color: #fff;
  }

  .btn-outline:hover:not(:disabled) {
    background: rgba(255, 255, 255, 0.12);
  }

  .btn:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }
</style>
