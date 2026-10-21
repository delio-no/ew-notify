<script lang="ts">
  let {
    isMonitoring = false,
    isBotRunning = false,
    isAdmin = false,
    captureMode = 'auto',
    discovered = '',
    packetsSeen = 0,
    matchesSeen = 0,
  }: {
    isMonitoring?: boolean;
    isBotRunning?: boolean;
    isAdmin?: boolean;
    captureMode?: string;
    discovered?: string;
    packetsSeen?: number;
    matchesSeen?: number;
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
</style>
