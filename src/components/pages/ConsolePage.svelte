<script lang="ts">
  import PageHeader from '../PageHeader.svelte';
  import type { PacketLogEntry, TargetWindow } from '../../lib/types';

  let {
    logLines = [],
    packetLines = [],
    packetLogEnabled = false,
    targetWindows = [],
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
    targetWindows?: TargetWindow[];
    consoleAutoScroll?: boolean;
    consoleEl?: HTMLPreElement | undefined;
    onRefreshLogs: () => void;
    onClearLogs: () => void;
    onTogglePacketLog: () => void;
    onRefreshPackets: () => void;
    onClearPackets: () => void;
    onCreatePattern: (hex: string) => void;
  } = $props();

  let selectedFilter = $state<number | 'all'>('all');

  let availableSlots = $derived.by(() => {
    const slots = new Set<number>();
    for (const p of packetLines) {
      if (typeof p.slot === 'number' && p.slot > 0) {
        slots.add(p.slot);
      }
    }
    for (let i = 0; i < targetWindows.length; i++) {
      slots.add(i + 1);
    }
    return Array.from(slots).sort((a, b) => a - b);
  });

  let filteredPackets = $derived.by(() => {
    if (selectedFilter === 'all') return packetLines;
    return packetLines.filter((p) => p.slot === selectedFilter);
  });

  function countForSlot(slot: number | 'all'): number {
    if (slot === 'all') return packetLines.length;
    return packetLines.filter((p) => p.slot === slot).length;
  }

  function getWindowInfoForSlot(slot: number) {
    const win = targetWindows[slot - 1];
    if (win) {
      return { pid: win.pid, title: win.title };
    }
    const pkt = packetLines.find((p) => p.slot === slot);
    if (pkt && pkt.pid) {
      return { pid: pkt.pid, title: pkt.window_title || '' };
    }
    return null;
  }
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
    <div class="card-header packet-header">
      <div class="header-main-row">
        <h2>
          Пакеты
          {#if packetLines.length > 0}
            <span class="header-count">({filteredPackets.length}{#if selectedFilter !== 'all'} из {packetLines.length}{/if})</span>
          {/if}
        </h2>
        <div class="actions">
          <label class="checkbox-label">
            <input type="checkbox" checked={packetLogEnabled} onchange={onTogglePacketLog} />
            <span>Писать пакеты</span>
          </label>
          <button type="button" class="btn btn-secondary btn-sm" onclick={onRefreshPackets}>Обновить</button>
          <button type="button" class="btn btn-danger btn-sm" onclick={onClearPackets}>Очистить</button>
        </div>
      </div>

      {#if packetLogEnabled && availableSlots.length > 0}
        <div class="filter-bar">
          <span class="filter-label">Фильтр по окну:</span>
          <div class="filter-pills">
            <button
              type="button"
              class="filter-pill {selectedFilter === 'all' ? 'active' : ''}"
              onclick={() => (selectedFilter = 'all')}
            >
              Все окна <span class="pill-badge">{countForSlot('all')}</span>
            </button>
            {#each availableSlots as slot}
              {@const info = getWindowInfoForSlot(slot)}
              <button
                type="button"
                class="filter-pill slot-{slot} {selectedFilter === slot ? 'active' : ''}"
                onclick={() => (selectedFilter = slot)}
              >
                <span class="pill-dot"></span>
                EW #{slot}{#if info?.pid}<span class="pill-pid">[{info.pid}]</span>{/if}
                <span class="pill-badge">{countForSlot(slot)}</span>
              </button>
            {/each}
          </div>
        </div>
      {/if}
    </div>
    <div class="card-body">
      {#if !packetLogEnabled}
        <p class="empty-text">Включи «Писать пакеты»</p>
      {:else if filteredPackets.length === 0}
        <p class="empty-text">
          {selectedFilter === 'all'
            ? 'Ожидание пакетов…'
            : `Нет пакетов для окна EW #${selectedFilter}`}
        </p>
      {:else}
        <div class="packet-list">
          {#each filteredPackets as p}
            <div class="packet-row">
              <div class="packet-row-meta">
                <span class="meta-time">{p.time}</span>
                {#if p.slot}
                  <span class="packet-slot-tag slot-{p.slot}">
                    EW #{p.slot}{#if p.pid} [{p.pid}]{/if}
                  </span>
                {:else if p.pid}
                  <span class="packet-slot-tag slot-unknown">
                    PID {p.pid}
                  </span>
                {/if}
                <span class="meta-endpoints">{p.remote} → {p.local}</span>
                <span class="meta-len">len={p.len}</span>
                <button
                  type="button"
                  class="btn btn-success btn-sm btn-pattern"
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
  .packet-header {
    flex-direction: column;
    align-items: stretch;
    gap: 8px;
  }

  .header-main-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    width: 100%;
  }

  .header-count {
    font-size: 13px;
    font-weight: 500;
    color: var(--text-muted, #94a3b8);
    margin-left: 4px;
  }

  .filter-bar {
    display: flex;
    align-items: center;
    gap: 10px;
    padding-top: 8px;
    border-top: 1px solid rgba(255, 255, 255, 0.08);
    flex-wrap: wrap;
  }

  .filter-label {
    font-size: 11px;
    color: var(--text-muted, #94a3b8);
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.05em;
  }

  .filter-pills {
    display: flex;
    gap: 6px;
    flex-wrap: wrap;
    align-items: center;
  }

  .filter-pill {
    background: rgba(255, 255, 255, 0.05);
    border: 1px solid rgba(255, 255, 255, 0.12);
    border-radius: 9999px;
    padding: 3px 10px;
    font-size: 12px;
    font-weight: 500;
    color: #e2e8f0;
    cursor: pointer;
    display: inline-flex;
    align-items: center;
    gap: 6px;
    transition: all 0.15s ease;
  }

  .filter-pill:hover {
    background: rgba(255, 255, 255, 0.1);
    border-color: rgba(255, 255, 255, 0.25);
  }

  .filter-pill.active {
    background: rgba(255, 255, 255, 0.18);
    border-color: #f8fafc;
    color: #ffffff;
    box-shadow: 0 0 10px rgba(255, 255, 255, 0.15);
  }

  .pill-dot {
    width: 7px;
    height: 7px;
    border-radius: 50%;
    background: currentColor;
  }

  .pill-pid {
    opacity: 0.75;
    font-size: 11px;
    font-family: monospace;
  }

  .pill-badge {
    background: rgba(0, 0, 0, 0.35);
    padding: 1px 6px;
    border-radius: 10px;
    font-size: 11px;
    font-weight: 600;
  }

  /* Slot color accents */
  .filter-pill.slot-1,
  .packet-slot-tag.slot-1 {
    color: #4ade80;
  }
  .filter-pill.slot-1.active {
    background: rgba(74, 222, 128, 0.18);
    border-color: #4ade80;
    box-shadow: 0 0 10px rgba(74, 222, 128, 0.25);
  }

  .filter-pill.slot-2,
  .packet-slot-tag.slot-2 {
    color: #38bdf8;
  }
  .filter-pill.slot-2.active {
    background: rgba(56, 189, 248, 0.18);
    border-color: #38bdf8;
    box-shadow: 0 0 10px rgba(56, 189, 248, 0.25);
  }

  .filter-pill.slot-3,
  .packet-slot-tag.slot-3 {
    color: #fbbf24;
  }
  .filter-pill.slot-3.active {
    background: rgba(251, 191, 36, 0.18);
    border-color: #fbbf24;
    box-shadow: 0 0 10px rgba(251, 191, 36, 0.25);
  }

  .filter-pill.slot-4,
  .packet-slot-tag.slot-4 {
    color: #c084fc;
  }
  .filter-pill.slot-4.active {
    background: rgba(192, 132, 252, 0.18);
    border-color: #c084fc;
    box-shadow: 0 0 10px rgba(192, 132, 252, 0.25);
  }

  .packet-slot-tag {
    font-size: 11px;
    font-weight: 700;
    padding: 1px 7px;
    border-radius: 4px;
    background: rgba(255, 255, 255, 0.07);
    border: 1px solid currentColor;
    letter-spacing: 0.02em;
    white-space: nowrap;
  }

  .packet-slot-tag.slot-unknown {
    color: #94a3b8;
    border-color: rgba(148, 163, 184, 0.4);
  }

  .packet-list {
    max-height: calc(100vh - 300px);
    overflow: auto;
  }

  .packet-row-meta {
    display: flex;
    align-items: center;
    gap: 8px;
    flex-wrap: wrap;
  }

  .meta-time {
    font-weight: 600;
  }

  .meta-endpoints {
    color: var(--text-muted, #94a3b8);
  }

  .btn-pattern {
    margin-left: auto;
  }
</style>
