<script lang="ts">
  import PageHeader from '../PageHeader.svelte';
  import type { PacketLogEntry } from '../../lib/types';

  let {
    logLines = [],
    packetLines = [],
    packetLogEnabled = false,
    consoleAutoScroll = $bindable(true),
    consoleEl = $bindable(undefined as HTMLPreElement | undefined),
    onRefreshLogs,
    onClearLogs,
    onTogglePacketLog,
    onRefreshPackets,
    onClearPackets,
    onCreatePattern,
  }: {
    logLines?: Array<{ time: string; message: string }>;
    packetLines?: PacketLogEntry[];
    packetLogEnabled?: boolean;
    consoleAutoScroll?: boolean;
    consoleEl?: HTMLPreElement | undefined;
    onRefreshLogs: () => void;
    onClearLogs: () => void;
    onTogglePacketLog: () => void;
    onRefreshPackets: () => void;
    onClearPackets: () => void;
    onCreatePattern: (hex: string) => void;
  } = $props();
</script>

<PageHeader title="Консоль">
  <label class="checkbox-label console-auto">
    <input type="checkbox" bind:checked={consoleAutoScroll} />
    <span>Автопрокрутка</span>
  </label>
  <button type="button" class="btn btn-secondary btn-sm" onclick={onRefreshLogs}>Обновить</button>
  <button type="button" class="btn btn-danger btn-sm" onclick={onClearLogs}>Очистить логи</button>
</PageHeader>

<div class="console-page">
  <div class="card console-card">
    <div class="card-header"><h2>Логи приложения</h2></div>
    <div class="card-body console-body">
      <pre class="console-output" bind:this={consoleEl}>
{#if logLines.length === 0}Нет записей.
{:else}{#each logLines as line}{line.time}  {line.message}
{/each}{/if}</pre>
    </div>
  </div>

  <div class="card console-card packet-card">
    <div class="card-header">
      <h2>Пакеты</h2>
      <div class="actions">
        <label class="checkbox-label">
          <input type="checkbox" checked={packetLogEnabled} onchange={onTogglePacketLog} />
          <span>Писать пакеты</span>
        </label>
        <button type="button" class="btn btn-secondary btn-sm" onclick={onRefreshPackets}>Обновить</button>
        <button type="button" class="btn btn-danger btn-sm" onclick={onClearPackets}>Очистить</button>
      </div>
    </div>
    <div class="card-body">
      {#if !packetLogEnabled}
        <p class="empty-text">Включи «Писать пакеты»</p>
      {:else if packetLines.length === 0}
        <p class="empty-text">Ожидание пакетов…</p>
      {:else}
        <div class="packet-list">
          {#each packetLines as p}
            <div class="packet-row">
              <div class="packet-row-meta">
                <span>{p.time}</span>
                <span>{p.remote} → {p.local}</span>
                <span>len={p.len}</span>
                <button
                  type="button"
                  class="btn btn-success btn-sm"
                  onclick={() => onCreatePattern(p.hex)}
                >
                  Паттерн
                </button>
              </div>
              <div class="packet-row-hex">{p.hex}</div>
            </div>
          {/each}
        </div>
      {/if}
    </div>
  </div>
</div>

<style>
  .packet-list {
    max-height: calc(100vh - 280px);
    overflow: auto;
  }
</style>
