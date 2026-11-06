<script lang="ts">
  import PageHeader from '../PageHeader.svelte';
  import StatusStrip from '../StatusStrip.svelte';
  import type { Config, StoredNotification, TargetWindow, WindowFilterMode } from '../../lib/types';

  let {
    config,
    isMonitoring,
    isBotRunning,
    isAdmin,
    captureMode,
    discoveredEndpoints,
    activePort,
    isCaptureAuto,
    packetsSeen,
    matchesSeen,
    dedupSuppressed,
    lastError,
    activeFilter,
    configPath,
    logPath,
    lastPollAt,
    pollErrors,
    monitorProcessName,
    notifications,
    windowFilterMode = { mode: 'All' },
    targetWindows = [],
    onStart,
    onStop,
    onHideTray,
    onSelectWindow,
    onRemoveSlot,
    onResetWindowFilter,
    onRefreshWindows,
  }: {
    config: Config;
    isMonitoring: boolean;
    isBotRunning: boolean;
    isAdmin: boolean;
    captureMode: string;
    discoveredEndpoints: string[];
    activePort: number | null;
    isCaptureAuto: boolean;
    packetsSeen: number;
    matchesSeen: number;
    dedupSuppressed: number;
    lastError: string;
    activeFilter: string;
    configPath: string;
    logPath: string;
    lastPollAt: string;
    pollErrors: number;
    monitorProcessName: string;
    notifications: StoredNotification[];
    windowFilterMode?: WindowFilterMode;
    targetWindows?: TargetWindow[];
    onStart: () => void;
    onStop: () => void;
    onHideTray: () => void;
    onSelectWindow: (slot: number, pid: number) => void;
    onRemoveSlot?: (slot: number) => void;
    onResetWindowFilter: () => void;
    onRefreshWindows: () => void;
  } = $props();

  let discoveredLabel = $derived(discoveredEndpoints.join(', '));

  function getWindowSlot(pid: number): number | null {
    if (!windowFilterMode) return null;
    if (windowFilterMode.mode === 'Single') {
      return windowFilterMode.data.pid === pid ? windowFilterMode.data.slot : null;
    }
    if (windowFilterMode.mode === 'Multi') {
      const match = windowFilterMode.data.find((d) => d.pid === pid);
      return match ? match.slot : null;
    }
    return null;
  }
</script>

<PageHeader title="Дашборд">
  <button type="button" class="btn btn-secondary btn-sm" onclick={onHideTray}>В трей</button>
</PageHeader>

<StatusStrip
  {isMonitoring}
  {isBotRunning}
  {isAdmin}
  {captureMode}
  discovered={discoveredLabel}
  {packetsSeen}
  {matchesSeen}
  {windowFilterMode}
/>

<div class="dashboard-grid">
  <div class="dashboard-main">
    <div class="card status-card">
      <div class="card-header"><h2>Управление</h2></div>
      <div class="card-body">
        <div class="status-display">
          <div class="status-indicator" class:active={isMonitoring}></div>
          <span class="status-text">
            {isMonitoring ? 'Мониторинг активен' : 'Мониторинг остановлен'}
          </span>
        </div>
        <div class="actions">
          {#if isMonitoring}
            <button class="btn btn-danger" onclick={onStop}>Остановить</button>
          {:else}
            <button class="btn btn-success" onclick={onStart}>Начать мониторинг</button>
          {/if}
        </div>
      </div>
    </div>

    <!-- Window Filtering (Multi-box) -->
    <div class="card window-filter-card">
      <div class="card-header window-filter-header">
        <div class="window-filter-title-wrap">
          <h2>Отслеживание окон (Мульти-бокс)</h2>
          <span class="hotkey-pill-hint">Ctrl + 1..6 / Ctrl + 0</span>
        </div>
        <button
          type="button"
          class="btn btn-secondary btn-xs"
          onclick={onRefreshWindows}
          title="Обновить список окон игры"
        >
          ↻ Обновить
        </button>
      </div>
      <div class="card-body">
        <!-- Active Filter Banner -->
        <div class="filter-banner" class:filter-active={windowFilterMode.mode !== 'All'}>
          <div class="filter-banner-info">
            <span class="filter-mode-tag">
              {#if windowFilterMode.mode === 'Single'}
                🟢 EW #{windowFilterMode.data.slot} [PID {windowFilterMode.data.pid}]
              {:else if windowFilterMode.mode === 'Multi'}
                🟢 Захвачено окон: {windowFilterMode.data.length} ({windowFilterMode.data.map(d => `EW #${d.slot}`).join(', ')})
              {:else}
                🌐 Все окна EndlessWar
              {/if}
            </span>
            <span class="filter-subtext">
              {#if windowFilterMode.mode === 'Single'}
                {windowFilterMode.data.title || 'Окно игры'} (оверлей активен)
              {:else if windowFilterMode.mode === 'Multi'}
                Раздельный мониторинг активен (оверлей на каждом захваченном окне)
              {:else}
                Пакеты и действия обрабатываются для всех окон игры
              {/if}
            </span>
          </div>
          {#if windowFilterMode.mode !== 'All'}
            <button
              type="button"
              class="btn btn-secondary btn-xs"
              onclick={onResetWindowFilter}
            >
              Сбросить все (Ctrl+0)
            </button>
          {/if}
        </div>

        <!-- Detected Windows List -->
        <div class="target-windows-section">
          <div class="section-subtitle">
            <span>Обнаруженные окна EndlessWar ({targetWindows.length})</span>
          </div>
          {#if targetWindows.length === 0}
            <div class="empty-windows-msg">
              Окна EndlessWar.exe не найдены. Запустите игру или нажмите <b>Ctrl + 1..6</b> в окне игры.
            </div>
          {:else}
            <div class="windows-grid">
              {#each targetWindows as win, idx}
                {@const trackedSlot = getWindowSlot(win.pid)}
                {@const isTracked = trackedSlot !== null}
                <div class="window-item" class:is-tracked={isTracked}>
                  <div class="window-meta">
                    <div class="window-title-row">
                      <span class="window-slot-indicator">#{idx + 1}</span>
                      <span class="window-title" title={win.title}>{win.title || 'EndlessWar'}</span>
                    </div>
                    <div class="window-details">
                      <span>PID: {win.pid}</span>
                      <span>{win.width}×{win.height}</span>
                      {#if win.is_minimized}
                        <span class="badge-minimized">Свёрнуто</span>
                      {/if}
                    </div>
                  </div>
                  <div class="window-actions">
                    {#if isTracked && trackedSlot !== null}
                      <span class="tracked-badge">✓ Слот #{trackedSlot}</span>
                      <button
                        type="button"
                        class="btn btn-secondary btn-xs"
                        onclick={() => onRemoveSlot ? onRemoveSlot(trackedSlot) : onResetWindowFilter()}
                        title="Снять привязку слота"
                      >
                        Снять
                      </button>
                    {:else}
                      <button
                        type="button"
                        class="btn btn-primary btn-xs"
                        onclick={() => onSelectWindow(idx + 1, win.pid)}
                        title={`Привязать как слот #${idx + 1}`}
                      >
                        Выбрать #{idx + 1}
                      </button>
                    {/if}
                  </div>
                </div>
              {/each}
            </div>
          {/if}
        </div>

        <!-- Hotkeys Cheatsheet -->
        <div class="hotkey-footer">
          <span class="hotkey-item"><kbd>Ctrl</kbd> + <kbd>1..6</kbd> привязать окно</span>
          <span class="hotkey-item"><kbd>Ctrl</kbd> + <kbd>0</kbd> сбросить на все окна</span>
        </div>
      </div>
    </div>

    <div class="card recent-card">
      <div class="card-header"><h2>Последние уведомления</h2></div>
      <div class="card-body">
        {#if notifications.length === 0}
          <p class="empty-text">Нет уведомлений</p>
        {:else}
          <ul class="notification-list">
            {#each notifications.slice(0, 5) as notif}
              <li>
                <span class="notif-time">{notif.time}</span>
                <div class="notif-main">
                  <span class="notif-title">{notif.title}</span>
                  {#if notif.body}
                    <span class="notif-meta">{notif.body}</span>
                  {/if}
                </div>
              </li>
            {/each}
          </ul>
        {/if}
      </div>
    </div>
  </div>

  <div class="card info-card">
    <div class="card-header"><h2>Захват</h2></div>
    <div class="card-body">
      <div class="info-stack">
        <div class="info-row">
          <span class="info-label">Режим</span>
          <span class="info-value">{captureMode === 'auto' ? 'Авто (процесс)' : 'Ручной'}</span>
        </div>
        <div class="info-row">
          <span class="info-label">Порт сервера</span>
          <span class="info-value"
            >{activePort ?? (isCaptureAuto ? '—' : config.target?.tcp_port) ?? '—'}</span
          >
        </div>
        {#if captureMode === 'auto'}
          <div class="info-row stacked">
            <span class="info-label">Discovered</span>
            <span class="info-value mono">{discoveredLabel || '—'}</span>
          </div>
        {/if}
        <div class="info-row">
          <span class="info-label">Фильтр окон</span>
          <span class="info-value">
            {#if windowFilterMode.mode === 'Single'}
              EW #{windowFilterMode.data.slot} (PID {windowFilterMode.data.pid})
            {:else}
              Все окна
            {/if}
          </span>
        </div>
        <div class="info-row">
          <span class="info-label">Dedup</span>
          <span class="info-value">{dedupSuppressed}</span>
        </div>
        {#if lastError}
          <div class="info-row stacked">
            <span class="info-label">Ошибка</span>
            <span class="info-value mono">{lastError}</span>
          </div>
        {/if}
      </div>

      <details class="debug-panel">
        <summary>Технические детали</summary>
        <div class="info-stack debug-stack">
          <div class="info-row stacked">
            <span class="info-label">Фильтр WinDivert</span>
            <span class="info-value mono">{activeFilter || '—'}</span>
          </div>
          <div class="info-row stacked">
            <span class="info-label">config.json</span>
            <span class="info-value mono">{configPath || '—'}</span>
          </div>
          <div class="info-row stacked">
            <span class="info-label">app.log</span>
            <span class="info-value mono">{logPath || '—'}</span>
          </div>
          <div class="info-row">
            <span class="info-label">Poll UI</span>
            <span class="info-value">{lastPollAt}</span>
          </div>
          <div class="info-row">
            <span class="info-label">Ошибки poll</span>
            <span class="info-value">{pollErrors}</span>
          </div>
          <div class="info-row">
            <span class="info-label">Процесс</span>
            <span class="info-value">{monitorProcessName || config.target?.process_name}</span>
          </div>
          <div class="info-row">
            <span class="info-label">Паттернов</span>
            <span class="info-value">{config.target?.patterns?.length || 0}</span>
          </div>
          <div class="info-row">
            <span class="info-label">Кулдаун</span>
            <span class="info-value">{config.notifications?.cooldown_seconds ?? 10} сек</span>
          </div>
          <div class="info-row">
            <span class="info-label">Burst dedup</span>
            <span class="info-value">{config.notifications?.burst_dedup_ms ?? 2000} мс</span>
          </div>
        </div>
      </details>
    </div>
  </div>
</div>

<style>
  .window-filter-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    gap: 12px;
  }
  .window-filter-title-wrap {
    display: flex;
    align-items: center;
    gap: 10px;
    flex-wrap: wrap;
  }
  .hotkey-pill-hint {
    font-size: 11px;
    padding: 2px 8px;
    border-radius: 999px;
    background: rgba(255, 255, 255, 0.06);
    color: var(--text-muted);
    border: 1px solid var(--border);
  }
  .filter-banner {
    display: flex;
    justify-content: space-between;
    align-items: center;
    gap: 12px;
    padding: 12px 14px;
    border-radius: 8px;
    background: var(--bg-input);
    border: 1px solid var(--border);
    margin-bottom: 14px;
  }
  .filter-banner.filter-active {
    background: rgba(34, 197, 94, 0.08);
    border-color: rgba(34, 197, 94, 0.35);
  }
  .filter-banner-info {
    display: flex;
    flex-direction: column;
    gap: 3px;
    min-width: 0;
  }
  .filter-mode-tag {
    font-weight: 600;
    font-size: 14px;
    color: #ffffff;
  }
  .filter-active .filter-mode-tag {
    color: #4ade80;
  }
  .filter-subtext {
    font-size: 12px;
    color: var(--text-dim);
  }
  .target-windows-section {
    display: flex;
    flex-direction: column;
    gap: 8px;
    margin-bottom: 14px;
  }
  .section-subtitle {
    font-size: 12px;
    font-weight: 600;
    color: var(--text-muted);
    text-transform: uppercase;
    letter-spacing: 0.5px;
  }
  .empty-windows-msg {
    font-size: 13px;
    color: var(--text-dim);
    padding: 12px;
    background: rgba(0, 0, 0, 0.2);
    border-radius: 8px;
    border: 1px dashed var(--border);
  }
  .windows-grid {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }
  .window-item {
    display: flex;
    justify-content: space-between;
    align-items: center;
    gap: 12px;
    padding: 10px 12px;
    background: rgba(0, 0, 0, 0.2);
    border-radius: 8px;
    border: 1px solid var(--border);
    transition: all 0.15s ease;
  }
  .window-item.is-tracked {
    border-color: rgba(34, 197, 94, 0.5);
    background: rgba(34, 197, 94, 0.06);
  }
  .window-meta {
    display: flex;
    flex-direction: column;
    gap: 3px;
    min-width: 0;
  }
  .window-title-row {
    display: flex;
    align-items: center;
    gap: 8px;
    min-width: 0;
  }
  .window-slot-indicator {
    font-size: 11px;
    font-weight: 700;
    color: var(--text-muted);
    background: rgba(255, 255, 255, 0.06);
    padding: 2px 6px;
    border-radius: 4px;
  }
  .window-title {
    font-size: 13px;
    font-weight: 500;
    color: var(--text);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    max-width: 240px;
  }
  .window-details {
    display: flex;
    align-items: center;
    gap: 10px;
    font-size: 11px;
    color: var(--text-dim);
  }
  .badge-minimized {
    color: var(--warn);
    background: rgba(240, 198, 116, 0.15);
    padding: 1px 5px;
    border-radius: 4px;
  }
  .window-actions {
    display: flex;
    align-items: center;
    gap: 8px;
    flex-shrink: 0;
  }
  .tracked-badge {
    font-size: 12px;
    font-weight: 600;
    color: #4ade80;
  }
  .btn-xs {
    padding: 5px 10px;
    font-size: 12px;
    border-radius: 6px;
  }
  .hotkey-footer {
    display: flex;
    flex-wrap: wrap;
    gap: 14px;
    padding-top: 10px;
    border-top: 1px solid var(--border);
    font-size: 12px;
    color: var(--text-dim);
  }
  .hotkey-item {
    display: flex;
    align-items: center;
    gap: 4px;
  }
  kbd {
    background: rgba(255, 255, 255, 0.08);
    border: 1px solid var(--border);
    border-radius: 4px;
    padding: 1px 5px;
    font-size: 11px;
    font-family: var(--mono);
    color: var(--text);
  }
</style>
