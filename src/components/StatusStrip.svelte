<script lang="ts">
  import type { WindowFilterMode } from '../lib/types';

  let {
    isMonitoring = false,
    isBotRunning = false,
    isAdmin = false,
    captureMode = 'auto',
    discovered = '',
    packetsSeen = 0,
    matchesSeen = 0,
    windowFilterMode = { mode: 'All' },
  }: {
    isMonitoring?: boolean;
    isBotRunning?: boolean;
    isAdmin?: boolean;
    captureMode?: string;
    discovered?: string;
    packetsSeen?: number;
    matchesSeen?: number;
    windowFilterMode?: WindowFilterMode;
  } = $props();
</script>

<div class="status-strip" aria-label="Статус">
  <span class="status-chip">
    <span class="dot" class:on={isMonitoring}></span>
    {isMonitoring ? 'Мониторинг' : 'Стоп'}
  </span>
  <span class="status-chip">
    <span class="dot" class:on={isBotRunning}></span>
    Бот {isBotRunning ? 'on' : 'off'}
  </span>
  <span class="status-chip">
    <span class="dot" class:on={isAdmin}></span>
    {isAdmin ? 'Admin' : 'No admin'}
  </span>
  <span class="status-chip">
    {captureMode === 'auto' ? 'Авто' : 'Ручной'}
  </span>
  {#if captureMode === 'auto' && discovered}
    <span class="status-chip mono">{discovered}</span>
  {/if}
  {#if windowFilterMode?.mode === 'Single'}
    <span class="status-chip active-filter" title={`Отслеживается окно #${windowFilterMode.data.slot} (PID: ${windowFilterMode.data.pid})`}>
      <span class="dot on"></span>
      EW #{windowFilterMode.data.slot} (PID {windowFilterMode.data.pid})
    </span>
  {:else}
    <span class="status-chip" title="Мониторинг всех запущенных окон">
      <span class="dot"></span>
      Все окна
    </span>
  {/if}
  <span class="status-chip">pkt {packetsSeen}</span>
  <span class="status-chip">evt {matchesSeen}</span>
</div>

<style>
  .mono {
    font-family: var(--mono);
    max-width: 280px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .active-filter {
    border-color: rgba(34, 197, 94, 0.4);
    background: rgba(34, 197, 94, 0.1);
    color: #4ade80;
    font-weight: 600;
  }
</style>
