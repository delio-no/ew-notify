<script lang="ts">
  import PageHeader from '../PageHeader.svelte';
  import StatusStrip from '../StatusStrip.svelte';
  import type { Config, StoredNotification } from '../../lib/types';

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
    onStart,
    onStop,
    onHideTray,
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
    onStart: () => void;
    onStop: () => void;
    onHideTray: () => void;
  } = $props();

  let discoveredLabel = $derived(discoveredEndpoints.join(', '));
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
            <span class="info-value">{config.notifications?.cooldown_seconds || 60} сек</span>
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
