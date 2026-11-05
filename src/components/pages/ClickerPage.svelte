<script lang="ts">
  import PageHeader from '../PageHeader.svelte';
  import type { ClickerConfig, ClickerStatus, Pattern } from '../../lib/types';

  let {
    clickerStatus,
    patterns = [],
    onStart,
    onStop,
    onSave,
    onTestKey,
  }: {
    clickerStatus: ClickerStatus;
    patterns?: Pattern[];
    onStart: () => void;
    onStop: () => void;
    onSave: (cfg: ClickerConfig) => void;
    onTestKey?: (key: string) => void;
  } = $props();

  let key = $state('1');
  let intervalMs = $state(1000);
  let stopOnPattern = $state(true);
  let stopPatterns = $state<string[]>([]);
  let notifyOnStop = $state(true);

  // Sync if status config changed externally
  $effect(() => {
    if (clickerStatus?.config) {
      key = clickerStatus.config.key;
      intervalMs = clickerStatus.config.interval_ms;
      stopOnPattern = clickerStatus.config.stop_on_pattern;
      stopPatterns = [...(clickerStatus.config.stop_patterns || [])];
      notifyOnStop = clickerStatus.config.notify_on_stop;
    }
  });

  const quickKeys = ['1', '2', '3', '4', 'F1', 'F2', 'F3', 'F4', 'Z', 'Space'];
  const quickIntervals = [200, 500, 1000, 2000, 3000, 5000];

  function togglePattern(patName: string) {
    if (stopPatterns.includes(patName)) {
      stopPatterns = stopPatterns.filter((p) => p !== patName);
    } else {
      stopPatterns = [...stopPatterns, patName];
    }
  }

  function handleSave() {
    onSave({
      key: key.trim().toUpperCase() || '1',
      interval_ms: Math.max(50, Number(intervalMs) || 1000),
      stop_on_pattern: stopOnPattern,
      stop_patterns: stopPatterns,
      notify_on_stop: notifyOnStop,
    });
  }
</script>

<PageHeader title="Автокликер" />

<div class="clicker-page">
  <!-- Status & Controls Hero Card -->
  <div class="card status-hero-card">
    <div class="status-hero-content">
      <div class="status-left">
        <div class="status-indicator-badge" class:active={clickerStatus.running}>
          <span class="status-dot"></span>
          <span class="status-label">
            {clickerStatus.running ? 'КЛИКЕР АКТИВЕН' : 'КЛИКЕР ОСТАНОВЛЕН'}
          </span>
        </div>

        <div class="stats-row">
          <div class="stat-pill">
            <span class="stat-label">Отправлено кликов:</span>
            <span class="stat-value">{clickerStatus.count}</span>
          </div>
          <div class="stat-pill">
            <span class="stat-label">Текущая клавиша:</span>
            <span class="stat-value key-badge">{clickerStatus.config.key}</span>
          </div>
          <div class="stat-pill">
            <span class="stat-label">Интервал:</span>
            <span class="stat-value">{clickerStatus.config.interval_ms} мс</span>
          </div>
        </div>

        {#if clickerStatus.last_stop_reason}
          <div class="stop-reason-alert">
            <span class="reason-icon">⏹</span>
            <span>Последняя остановка: <strong>{clickerStatus.last_stop_reason}</strong></span>
          </div>
        {/if}
      </div>

      <div class="status-actions">
        {#if !clickerStatus.running}
          <button type="button" class="btn btn-primary btn-hero" onclick={onStart}>
            ▶ Запустить (Ctrl + O)
          </button>
        {:else}
          <button type="button" class="btn btn-danger btn-hero" onclick={onStop}>
            ⏹ Остановить (Ctrl + P)
          </button>
        {/if}
      </div>
    </div>

    <div class="hotkey-banner">
      <span class="hotkey-icon">⌨</span>
      <span>
        Глобальные горячие клавиши: <strong>Ctrl + O</strong> — запуск, <strong>Ctrl + P</strong> — выключение.
        Кликер отправляет нажатия в фоновом режиме без перехвата фокуса.
      </span>
    </div>
  </div>

  <div class="clicker-grid">
    <!-- Clicker Parameters Card -->
    <div class="card">
      <div class="card-header">
        <h2>Параметры клика</h2>
      </div>
      <div class="card-body">
        <!-- Key selection -->
        <div class="form-group">
          <label for="clicker-key">Клавиша для нажатия</label>
          <div class="key-input-row">
            <input
              id="clicker-key"
              type="text"
              class="input-key"
              bind:value={key}
              placeholder="1, 2, F1, Z, Space"
              maxlength="10"
            />
            {#if onTestKey}
              <button
                type="button"
                class="btn btn-secondary btn-sm"
                onclick={() => onTestKey(key)}
                title="Отправить клавишу в окно прямо сейчас"
              >
                Тест
              </button>
            {/if}
          </div>

          <div class="quick-chips">
            <span class="chips-label">Быстрый выбор:</span>
            {#each quickKeys as qk}
              <button
                type="button"
                class="chip-btn"
                class:active={key.toUpperCase() === qk.toUpperCase()}
                onclick={() => (key = qk)}
              >
                {qk}
              </button>
            {/each}
          </div>
        </div>

        <!-- Interval selection -->
        <div class="form-group">
          <label for="clicker-interval">
            Интервал отправки (миллисекунды)
            <span class="hint-text">({intervalMs} мс = {(intervalMs / 1000).toFixed(1)} сек)</span>
          </label>
          <input
            id="clicker-interval"
            type="number"
            min="50"
            max="600000"
            step="50"
            bind:value={intervalMs}
          />

          <div class="quick-chips">
            <span class="chips-label">Быстрый выбор:</span>
            {#each quickIntervals as qi}
              <button
                type="button"
                class="chip-btn"
                class:active={Number(intervalMs) === qi}
                onclick={() => (intervalMs = qi)}
              >
                {qi} мс
              </button>
            {/each}
          </div>
        </div>

        <button type="button" class="btn btn-primary" onclick={handleSave}>
          Сохранить настройки
        </button>
      </div>
    </div>

    <!-- Pattern Integration Card -->
    <div class="card">
      <div class="card-header">
        <h2>Автоостановка по паттернам пакетов</h2>
      </div>
      <div class="card-body">
        <div class="form-group">
          <label class="checkbox-label">
            <input type="checkbox" bind:checked={stopOnPattern} />
            <span><strong>Останавливать кликер при обнаружении паттерна</strong></span>
          </label>
          <p class="field-hint">
            При срабатывании сетевого паттерна (например, смерть, телепорт или снятие баффа)
            кликер мгновенно выключится.
          </p>
        </div>

        {#if stopOnPattern}
          <div class="pattern-selection-box">
            <div class="pattern-box-header">
              <span>Паттерны, вызывающие остановку:</span>
              <span class="subtext">
                {stopPatterns.length === 0 ? 'Останавливать при любом паттерне' : `Выбрано: ${stopPatterns.length}`}
              </span>
            </div>

            <div class="pattern-chips-list">
              <button
                type="button"
                class="pattern-toggle-chip"
                class:selected={stopPatterns.length === 0}
                onclick={() => (stopPatterns = [])}
              >
                ⚡ Любой паттерн
              </button>
              {#each patterns as pat}
                <button
                  type="button"
                  class="pattern-toggle-chip"
                  class:selected={stopPatterns.includes(pat.name)}
                  onclick={() => togglePattern(pat.name)}
                >
                  {pat.name}
                </button>
              {/each}
            </div>
          </div>

          <div class="form-group" style="margin-top: 16px;">
            <label class="checkbox-label">
              <input type="checkbox" bind:checked={notifyOnStop} />
              <span>Показывать системное уведомление при автоостановке</span>
            </label>
          </div>
        {/if}

        <button type="button" class="btn btn-primary" onclick={handleSave} style="margin-top: 12px;">
          Сохранить настройки
        </button>
      </div>
    </div>
  </div>
</div>

<style>
  .clicker-page {
    display: flex;
    flex-direction: column;
    gap: 1.25rem;
  }

  .status-hero-card {
    background: var(--color-surface, #1e1e2e);
    border: 1px solid var(--color-border, #313244);
    border-radius: 12px;
    padding: 1.25rem;
    display: flex;
    flex-direction: column;
    gap: 1rem;
  }

  .status-hero-content {
    display: flex;
    justify-content: space-between;
    align-items: center;
    flex-wrap: wrap;
    gap: 1.25rem;
  }

  .status-left {
    display: flex;
    flex-direction: column;
    gap: 0.75rem;
  }

  .status-indicator-badge {
    display: inline-flex;
    align-items: center;
    gap: 8px;
    padding: 6px 14px;
    border-radius: 20px;
    background: rgba(108, 112, 134, 0.2);
    border: 1px solid rgba(108, 112, 134, 0.4);
    font-weight: 700;
    font-size: 0.9rem;
    letter-spacing: 0.5px;
    color: #a6adc8;
    width: fit-content;
  }

  .status-indicator-badge.active {
    background: rgba(166, 227, 161, 0.15);
    border-color: rgba(166, 227, 161, 0.5);
    color: #a6e3a1;
  }

  .status-dot {
    width: 10px;
    height: 10px;
    border-radius: 50%;
    background: #6c7086;
  }

  .status-indicator-badge.active .status-dot {
    background: #a6e3a1;
    box-shadow: 0 0 10px #a6e3a1;
    animation: pulse-dot 1.5s infinite;
  }

  @keyframes pulse-dot {
    0% {
      opacity: 1;
      transform: scale(1);
    }
    50% {
      opacity: 0.5;
      transform: scale(1.2);
    }
    100% {
      opacity: 1;
      transform: scale(1);
    }
  }

  .stats-row {
    display: flex;
    gap: 0.75rem;
    flex-wrap: wrap;
  }

  .stat-pill {
    background: rgba(255, 255, 255, 0.04);
    border: 1px solid var(--color-border, #313244);
    border-radius: 8px;
    padding: 6px 12px;
    font-size: 0.85rem;
    display: flex;
    align-items: center;
    gap: 6px;
  }

  .stat-label {
    color: #a6adc8;
  }

  .stat-value {
    font-weight: 700;
    color: #cdd6f4;
  }

  .key-badge {
    background: #45475a;
    padding: 2px 8px;
    border-radius: 4px;
    font-family: monospace;
    font-size: 0.95rem;
  }

  .stop-reason-alert {
    display: flex;
    align-items: center;
    gap: 8px;
    background: rgba(243, 139, 168, 0.1);
    border: 1px solid rgba(243, 139, 168, 0.3);
    color: #f38ba8;
    padding: 6px 12px;
    border-radius: 6px;
    font-size: 0.85rem;
  }

  .btn-hero {
    padding: 12px 24px;
    font-size: 1.05rem;
    font-weight: 700;
    border-radius: 8px;
    cursor: pointer;
  }

  .hotkey-banner {
    display: flex;
    align-items: center;
    gap: 10px;
    background: rgba(137, 180, 250, 0.08);
    border: 1px solid rgba(137, 180, 250, 0.2);
    border-radius: 8px;
    padding: 8px 14px;
    font-size: 0.85rem;
    color: #b4befe;
  }

  .clicker-grid {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(320px, 1fr));
    gap: 1.25rem;
  }

  .key-input-row {
    display: flex;
    gap: 8px;
    align-items: center;
  }

  .input-key {
    width: 140px;
    text-transform: uppercase;
    font-weight: 700;
    font-family: monospace;
    font-size: 1.1rem;
    text-align: center;
  }

  .quick-chips {
    display: flex;
    align-items: center;
    gap: 6px;
    flex-wrap: wrap;
    margin-top: 8px;
  }

  .chips-label {
    font-size: 0.8rem;
    color: #a6adc8;
    margin-right: 2px;
  }

  .chip-btn {
    background: rgba(255, 255, 255, 0.05);
    border: 1px solid var(--color-border, #313244);
    border-radius: 6px;
    color: #cdd6f4;
    padding: 3px 8px;
    font-size: 0.8rem;
    cursor: pointer;
    transition: all 0.15s;
  }

  .chip-btn:hover {
    background: rgba(255, 255, 255, 0.12);
  }

  .chip-btn.active {
    background: #89b4fa;
    color: #11111b;
    border-color: #89b4fa;
    font-weight: 700;
  }

  .hint-text {
    font-size: 0.8rem;
    color: #a6adc8;
    font-weight: normal;
    margin-left: 6px;
  }

  .field-hint {
    font-size: 0.8rem;
    color: #a6adc8;
    margin-top: 4px;
  }

  .pattern-selection-box {
    background: rgba(0, 0, 0, 0.2);
    border: 1px solid var(--color-border, #313244);
    border-radius: 8px;
    padding: 10px;
    margin-top: 10px;
  }

  .pattern-box-header {
    display: flex;
    justify-content: space-between;
    font-size: 0.85rem;
    color: #cdd6f4;
    margin-bottom: 8px;
  }

  .subtext {
    color: #a6adc8;
    font-size: 0.8rem;
  }

  .pattern-chips-list {
    display: flex;
    gap: 6px;
    flex-wrap: wrap;
  }

  .pattern-toggle-chip {
    background: rgba(255, 255, 255, 0.05);
    border: 1px solid var(--color-border, #313244);
    border-radius: 6px;
    color: #a6adc8;
    padding: 4px 10px;
    font-size: 0.8rem;
    cursor: pointer;
    transition: all 0.15s;
  }

  .pattern-toggle-chip:hover {
    background: rgba(255, 255, 255, 0.1);
    color: #cdd6f4;
  }

  .pattern-toggle-chip.selected {
    background: rgba(249, 226, 175, 0.15);
    border-color: #f9e2af;
    color: #f9e2af;
    font-weight: 700;
  }
</style>
