<script lang="ts">
  import { untrack } from 'svelte';
  import PageHeader from '../PageHeader.svelte';
  import KeyRecorder from '../KeyRecorder.svelte';
  import type {
    ClickerConfig,
    ClickerRule,
    ClickerStatus,
    ClickerTarget,
    Pattern,
    WindowFilterMode,
  } from '../../lib/types';

  let {
    clickerStatus,
    patterns = [],
    windowFilterMode,
    onStart,
    onStop,
    onSave,
    onTestKey,
  }: {
    clickerStatus: ClickerStatus;
    patterns?: Pattern[];
    windowFilterMode?: WindowFilterMode;
    onStart: () => void;
    onStop: () => void;
    onSave: (cfg: ClickerConfig) => void;
    onTestKey?: (key: string, target?: ClickerTarget) => void;
  } = $props();

  function parseInitialRules(rawRules?: ClickerRule[]): ClickerRule[] {
    if (rawRules && rawRules.length > 0) {
      return rawRules.map((r, i) => ({
        ...r,
        id: r.id || `rule-${i + 1}`,
        name: r.name || `Клавиша ${i + 1}`,
        enabled: r.enabled !== false,
        key: r.key || '1',
        interval_ms: Number(r.interval_ms) || 1000,
        target: r.target || { type: 'Captured' },
        stop_on_pattern: r.stop_on_pattern !== false,
        stop_patterns: [...(r.stop_patterns || [])],
      }));
    }
    return [
      {
        id: 'rule-1',
        name: 'Клавиша 1',
        enabled: true,
        key: '1',
        interval_ms: 1000,
        target: { type: 'Captured' },
        stop_on_pattern: true,
        stop_patterns: [],
      },
    ];
  }

  let rules = $state<ClickerRule[]>([]);
  let notifyOnStop = $state(true);

  // Sync safely when clickerStatus config changes externally without reactive feedback loops
  $effect(() => {
    const externalRules = clickerStatus?.config?.rules;
    const externalNotify = clickerStatus?.config?.notify_on_stop;
    untrack(() => {
      rules = parseInitialRules(externalRules);
      if (externalNotify !== undefined) {
        notifyOnStop = externalNotify;
      }
    });
  });

  const quickKeys = ['1', '2', '3', '4', 'F1', 'F2', 'F3', 'F4', 'Z', 'Space'];
  const quickIntervals = [200, 500, 1000, 2000, 3000, 5000];

  function getTargetValue(target?: ClickerTarget): string {
    if (!target) return 'Captured';
    if (target.type === 'Slot') {
      return `Slot-${target.data ?? 1}`;
    }
    return target.type || 'Captured';
  }

  function setTargetValue(rule: ClickerRule, val: string) {
    if (val === 'Captured') {
      rule.target = { type: 'Captured' };
    } else if (val === 'All') {
      rule.target = { type: 'All' };
    } else if (val.startsWith('Slot-')) {
      const slotNum = parseInt(val.replace('Slot-', ''), 10) || 1;
      rule.target = { type: 'Slot', data: slotNum };
    }
  }

  function addRule() {
    const nextNum = rules.length + 1;
    const newRule: ClickerRule = {
      id: `rule-${Date.now()}-${Math.random().toString(36).slice(2, 6)}`,
      name: `Клавиша ${nextNum}`,
      enabled: true,
      key: nextNum <= 9 ? `${nextNum}` : '1',
      interval_ms: 1000,
      target: { type: 'Captured' },
      stop_on_pattern: true,
      stop_patterns: [],
    };
    rules = [...rules, newRule];
  }

  function removeRule(id: string) {
    if (rules.length <= 1) {
      rules = [
        {
          id: `rule-${Date.now()}`,
          name: 'Клавиша 1',
          enabled: true,
          key: '1',
          interval_ms: 1000,
          target: { type: 'Captured' },
          stop_on_pattern: true,
          stop_patterns: [],
        },
      ];
      return;
    }
    rules = rules.filter((r) => r.id !== id);
  }

  function toggleRulePattern(rule: ClickerRule, patName: string) {
    if (!rule.stop_patterns) rule.stop_patterns = [];
    if (rule.stop_patterns.includes(patName)) {
      rule.stop_patterns = rule.stop_patterns.filter((p) => p !== patName);
    } else {
      rule.stop_patterns = [...rule.stop_patterns, patName];
    }
  }

  function handleKeyInput(e: KeyboardEvent, rule: ClickerRule) {
    // Ignore standalone modifier presses
    if (
      ['Shift', 'Control', 'Alt', 'Meta'].includes(e.key) ||
      ['ShiftLeft', 'ShiftRight', 'ControlLeft', 'ControlRight', 'AltLeft', 'AltRight', 'MetaLeft', 'MetaRight'].includes(e.code)
    ) {
      return;
    }

    e.preventDefault();
    e.stopPropagation();

    const code = e.code;
    let keyName: string | null = null;

    if (/^F([1-9]|1[0-9]|2[0-4])$/i.test(code)) {
      keyName = code.toUpperCase();
    } else if (code.startsWith('Key')) {
      keyName = code.slice(3).toUpperCase();
    } else if (code.startsWith('Digit')) {
      keyName = code.slice(5);
    } else if (code.startsWith('Numpad') && code.length === 7 && !isNaN(Number(code[6]))) {
      keyName = `Numpad${code[6]}`;
    } else {
      switch (code) {
        case 'Space':
          keyName = 'Space';
          break;
        case 'Enter':
        case 'NumpadEnter':
          keyName = 'Enter';
          break;
        case 'Escape':
          keyName = 'Escape';
          break;
        case 'Tab':
          keyName = 'Tab';
          break;
        case 'Backspace':
          keyName = 'Backspace';
          break;
        case 'Delete':
          keyName = 'Delete';
          break;
        case 'Insert':
          keyName = 'Insert';
          break;
        case 'Home':
          keyName = 'Home';
          break;
        case 'End':
          keyName = 'End';
          break;
        case 'PageUp':
          keyName = 'PageUp';
          break;
        case 'PageDown':
          keyName = 'PageDown';
          break;
        case 'ArrowUp':
          keyName = 'Up';
          break;
        case 'ArrowDown':
          keyName = 'Down';
          break;
        case 'ArrowLeft':
          keyName = 'Left';
          break;
        case 'ArrowRight':
          keyName = 'Right';
          break;
        default:
          if (e.key && e.key.length === 1 && e.key.trim()) {
            keyName = e.key.toUpperCase();
          }
          break;
      }
    }

    if (keyName) {
      rule.key = keyName;
    }
  }

  function handleSave() {
    const cleanedRules = rules.map((r) => ({
      ...r,
      key: r.key.trim().toUpperCase() || '1',
      interval_ms: Math.max(50, Number(r.interval_ms) || 1000),
      stop_patterns: [...r.stop_patterns],
    }));

    onSave({
      rules: cleanedRules,
      notify_on_stop: notifyOnStop,
    });
  }
</script>

<PageHeader title="Автокликер">
  <button type="button" class="btn btn-secondary btn-sm" onclick={addRule}>
    + Добавить клавишу
  </button>
  <button type="button" class="btn btn-primary btn-sm" onclick={handleSave}>
    Сохранить настройки
  </button>
</PageHeader>

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
            <span class="stat-label">Всего кликов:</span>
            <span class="stat-value">{clickerStatus.count}</span>
          </div>

          <div class="stat-pill">
            <span class="stat-label">Активных правил:</span>
            <span class="stat-value">
              {rules.filter((r) => r.enabled).length} из {rules.length}
            </span>
          </div>
        </div>

        {#if clickerStatus.last_stop_reason}
          <div class="stop-reason-alert">
            <span class="reason-icon">⏹</span>
            <span>{clickerStatus.last_stop_reason}</span>
          </div>
        {/if}
      </div>

      <div class="status-actions">
        {#if !clickerStatus.running}
          <button type="button" class="btn btn-primary btn-hero" onclick={onStart}>
            ▶ Запустить все (Ctrl + O)
          </button>
        {:else}
          <button type="button" class="btn btn-danger btn-hero" onclick={onStop}>
            ⏹ Остановить все (Ctrl + P)
          </button>
        {/if}
      </div>
    </div>

    <!-- Captured Window info -->
    {#if windowFilterMode?.mode === 'Single'}
      <div class="target-window-banner captured">
        <span class="banner-icon">🎯</span>
        <span>
          Текущее захваченное окно: <strong>Слот #{windowFilterMode.data.slot} [PID {windowFilterMode.data.pid}]</strong> — {windowFilterMode.data.title}
        </span>
      </div>
    {:else}
      <div class="target-window-banner warning">
        <span class="banner-icon">⚠️</span>
        <span>
          Окно игры не захвачено (режим «Все окна»). Для правил с целью <em>«Захваченное окно»</em> нажмите <strong>Ctrl + 1..6</strong> в окне игры, либо выберите конкретный слот в правиле.
        </span>
      </div>
    {/if}

    <div class="hotkey-banner">
      <span class="hotkey-icon">⌨</span>
      <span>
        Горячие клавиши: <strong>Ctrl + O</strong> — запуск всех включенных клавиш, <strong>Ctrl + P</strong> — остановка. Каждая клавиша работает независимо в фоновом режиме.
      </span>
    </div>
  </div>

  <!-- Rules List Header -->
  <div class="rules-section-header">
    <div class="rules-header-title">
      <h2>Настроенные клавиши ({rules.length})</h2>
      <p class="section-desc">У каждой клавиши свой независимый интервал, целевое окно и автоостановка по паттернам</p>
    </div>
    <div class="rules-header-actions">
      <button type="button" class="btn btn-secondary" onclick={addRule}>
        + Добавить клавишу
      </button>
      <button type="button" class="btn btn-primary" onclick={handleSave}>
        Сохранить все
      </button>
    </div>
  </div>

  <!-- Rules Cards List -->
  <div class="rules-list">
    {#each rules as rule, idx (rule.id)}
      <div class="card rule-card" class:disabled={!rule.enabled}>
        <div class="rule-card-header">
          <div class="rule-header-left">
            <label class="switch-container" title="Включить / выключить эту клавишу">
              <input type="checkbox" bind:checked={rule.enabled} />
              <span class="switch-slider"></span>
            </label>

            <span class="rule-index">#{idx + 1}</span>

            <input
              type="text"
              class="rule-name-input"
              bind:value={rule.name}
              placeholder="Название действия (например: Скилл 1)"
            />
          </div>

          <div class="rule-header-right">
            <span class="rule-key-preview">{rule.key || '?'}</span>
            <span class="rule-interval-preview">{rule.interval_ms} мс</span>

            {#if onTestKey}
              <button
                type="button"
                class="btn btn-secondary btn-sm"
                onclick={() => onTestKey(rule.key, rule.target)}
                title="Отправить эту клавишу прямо сейчас в её целевое окно"
              >
                Тест
              </button>
            {/if}

            <button
              type="button"
              class="btn-delete-rule"
              onclick={() => removeRule(rule.id)}
              title="Удалить это правило"
            >
              🗑
            </button>
          </div>
        </div>

        <div class="rule-card-body">
          <div class="rule-fields-grid">
            <!-- Key Field -->
            <div class="form-group">
              <label for="rule-key-{rule.id}">
                Клавиша
                <span class="hint-text">(нажмите кнопку или поле и клавишу F1-F12, 1-9...)</span>
              </label>
              <div class="key-input-row">
                <KeyRecorder bind:value={rule.key} />
                <input
                  id="rule-key-{rule.id}"
                  type="text"
                  class="rule-key-input"
                  bind:value={rule.key}
                  placeholder="F1..."
                  maxlength="10"
                  onkeydown={(e) => handleKeyInput(e, rule)}
                  title="Кликните сюда и нажмите любую клавишу на клавиатуре (F1-F12, Space, буквы, цифры)"
                />
              </div>
              <div class="quick-chips">
                {#each quickKeys as qk}
                  <button
                    type="button"
                    class="chip-btn"
                    class:active={rule.key.toUpperCase() === qk.toUpperCase()}
                    onclick={() => (rule.key = qk)}
                  >
                    {qk}
                  </button>
                {/each}
              </div>
            </div>

            <!-- Interval Field -->
            <div class="form-group">
              <label for="rule-interval-{rule.id}">
                Интервал (мс)
                <span class="hint-text">({rule.interval_ms} мс = {(rule.interval_ms / 1000).toFixed(1)} с)</span>
              </label>
              <input
                id="rule-interval-{rule.id}"
                type="number"
                min="50"
                max="600000"
                step="50"
                bind:value={rule.interval_ms}
              />
              <div class="quick-chips">
                {#each quickIntervals as qi}
                  <button
                    type="button"
                    class="chip-btn"
                    class:active={Number(rule.interval_ms) === qi}
                    onclick={() => (rule.interval_ms = qi)}
                  >
                    {qi} мс
                  </button>
                {/each}
              </div>
            </div>

            <!-- Target Window Field -->
            <div class="form-group">
              <label for="rule-target-{rule.id}">Целевое окно игры</label>
              <select
                id="rule-target-{rule.id}"
                class="rule-target-select"
                value={getTargetValue(rule.target)}
                onchange={(e) => setTargetValue(rule, e.currentTarget.value)}
              >
                <option value="Captured">🎯 Захваченное окно (Ctrl+1..6)</option>
                <option value="Slot-1">🟢 Окно Слот #1</option>
                <option value="Slot-2">🟢 Окно Слот #2</option>
                <option value="Slot-3">🟢 Окно Слот #3</option>
                <option value="Slot-4">🟢 Окно Слот #4</option>
                <option value="Slot-5">🟢 Окно Слот #5</option>
                <option value="Slot-6">🟢 Окно Слот #6</option>
                <option value="All">🌐 Все окна игры</option>
              </select>
              <p class="field-hint">
                {#if rule.target.type === 'Captured'}
                  Отправляет только в окно, захваченное через Ctrl+1..6.
                {:else if rule.target.type === 'Slot'}
                  Привязано строго к окну слота #{rule.target.data}.
                {:else}
                  Рассылает клавишу во все запущенные окна игры.
                {/if}
              </p>
            </div>
          </div>

          <!-- Pattern Stop Configuration for this Rule -->
          <div class="rule-pattern-section">
            <label class="checkbox-label">
              <input type="checkbox" bind:checked={rule.stop_on_pattern} />
              <span>Останавливать эту клавишу при обнаружении сетевого паттерна</span>
            </label>

            {#if rule.stop_on_pattern}
              <div class="rule-pattern-chips">
                <span class="chips-label">Паттерны:</span>
                <button
                  type="button"
                  class="pattern-chip"
                  class:active={!rule.stop_patterns || rule.stop_patterns.length === 0}
                  onclick={() => (rule.stop_patterns = [])}
                >
                  ⚡ Любой паттерн
                </button>
                {#each patterns as pat}
                  <button
                    type="button"
                    class="pattern-chip"
                    class:active={(rule.stop_patterns || []).includes(pat.name)}
                    onclick={() => toggleRulePattern(rule, pat.name)}
                  >
                    {pat.name}
                  </button>
                {/each}
              </div>
            {/if}
          </div>
        </div>
      </div>
    {/each}
  </div>

  <!-- General Settings / Notification Card -->
  <div class="card footer-settings-card">
    <div class="footer-settings-row">
      <label class="checkbox-label">
        <input type="checkbox" bind:checked={notifyOnStop} />
        <span>Показывать системное уведомление при автоостановке клавиш по паттерну</span>
      </label>

      <button type="button" class="btn btn-primary" onclick={handleSave}>
        Сохранить настройки
      </button>
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

  .target-window-banner {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 10px 14px;
    border-radius: 8px;
    font-size: 0.88rem;
    line-height: 1.4;
  }

  .target-window-banner.captured {
    background: rgba(166, 227, 161, 0.1);
    border: 1px solid rgba(166, 227, 161, 0.3);
    color: #a6e3a1;
  }

  .target-window-banner.warning {
    background: rgba(249, 226, 175, 0.1);
    border: 1px solid rgba(249, 226, 175, 0.35);
    color: #f9e2af;
  }

  .banner-icon {
    font-size: 1.15rem;
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

  .rules-section-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    flex-wrap: wrap;
    gap: 1rem;
    margin-top: 0.5rem;
  }

  .rules-header-title h2 {
    font-size: 1.2rem;
    margin: 0;
    color: #cdd6f4;
  }

  .section-desc {
    font-size: 0.85rem;
    color: #a6adc8;
    margin: 4px 0 0 0;
  }

  .rules-header-actions {
    display: flex;
    gap: 8px;
  }

  .rules-list {
    display: flex;
    flex-direction: column;
    gap: 1rem;
  }

  .rule-card {
    background: var(--color-surface, #1e1e2e);
    border: 1px solid var(--color-border, #313244);
    border-radius: 10px;
    overflow: hidden;
    transition: opacity 0.2s, border-color 0.2s;
  }

  .rule-card.disabled {
    opacity: 0.6;
    border-style: dashed;
  }

  .rule-card-header {
    background: rgba(255, 255, 255, 0.03);
    border-bottom: 1px solid var(--color-border, #313244);
    padding: 10px 14px;
    display: flex;
    justify-content: space-between;
    align-items: center;
    flex-wrap: wrap;
    gap: 10px;
  }

  .rule-header-left {
    display: flex;
    align-items: center;
    gap: 12px;
    flex: 1;
    min-width: 250px;
  }

  .rule-index {
    font-family: monospace;
    font-weight: 700;
    color: #89b4fa;
    font-size: 0.95rem;
  }

  .rule-name-input {
    background: transparent;
    border: 1px solid transparent;
    border-radius: 4px;
    color: #cdd6f4;
    font-weight: 700;
    font-size: 1rem;
    padding: 4px 8px;
    flex: 1;
    max-width: 320px;
  }

  .rule-name-input:focus {
    background: rgba(0, 0, 0, 0.2);
    border-color: #89b4fa;
  }

  .rule-header-right {
    display: flex;
    align-items: center;
    gap: 10px;
  }

  .rule-key-preview {
    background: #45475a;
    padding: 3px 8px;
    border-radius: 4px;
    font-family: monospace;
    font-size: 0.9rem;
    font-weight: 700;
    color: #89b4fa;
  }

  .rule-interval-preview {
    background: rgba(255, 255, 255, 0.05);
    border: 1px solid var(--color-border, #313244);
    padding: 3px 8px;
    border-radius: 4px;
    font-size: 0.82rem;
    color: #a6adc8;
  }

  .btn-delete-rule {
    background: transparent;
    border: none;
    color: #6c7086;
    font-size: 1.1rem;
    cursor: pointer;
    padding: 4px 8px;
    border-radius: 4px;
    transition: color 0.15s, background 0.15s;
  }

  .btn-delete-rule:hover {
    color: #f38ba8;
    background: rgba(243, 139, 168, 0.15);
  }

  .rule-card-body {
    padding: 14px;
    display: flex;
    flex-direction: column;
    gap: 12px;
  }

  .rule-fields-grid {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(260px, 1fr));
    gap: 14px;
  }

  .key-input-row {
    display: flex;
    align-items: center;
    gap: 8px;
    flex-wrap: wrap;
  }

  .rule-key-input {
    width: 80px;
    font-weight: 700;
    font-family: monospace;
    font-size: 1rem;
    text-transform: uppercase;
    text-align: center;
  }

  .rule-target-select {
    width: 100%;
    background: #181825;
    border: 1px solid var(--color-border, #313244);
    color: #cdd6f4;
    border-radius: 6px;
    padding: 8px;
    font-size: 0.88rem;
  }

  .rule-pattern-section {
    background: rgba(0, 0, 0, 0.15);
    border: 1px solid var(--color-border, #313244);
    border-radius: 8px;
    padding: 10px 14px;
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  .rule-pattern-chips {
    display: flex;
    align-items: center;
    gap: 6px;
    flex-wrap: wrap;
    margin-top: 4px;
  }

  .pattern-chip {
    background: rgba(255, 255, 255, 0.05);
    border: 1px solid var(--color-border, #313244);
    border-radius: 6px;
    color: #a6adc8;
    padding: 3px 8px;
    font-size: 0.8rem;
    cursor: pointer;
    transition: all 0.15s;
  }

  .pattern-chip:hover {
    background: rgba(255, 255, 255, 0.1);
    color: #cdd6f4;
  }

  .pattern-chip.active {
    background: rgba(249, 226, 175, 0.18);
    border-color: #f9e2af;
    color: #f9e2af;
    font-weight: 700;
  }

  .quick-chips {
    display: flex;
    align-items: center;
    gap: 4px;
    flex-wrap: wrap;
    margin-top: 6px;
  }

  .chips-label {
    font-size: 0.8rem;
    color: #a6adc8;
    margin-right: 4px;
  }

  .chip-btn {
    background: rgba(255, 255, 255, 0.05);
    border: 1px solid var(--color-border, #313244);
    border-radius: 5px;
    color: #cdd6f4;
    padding: 2px 7px;
    font-size: 0.78rem;
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

  /* Custom toggle switch */
  .switch-container {
    position: relative;
    display: inline-block;
    width: 36px;
    height: 20px;
    cursor: pointer;
  }

  .switch-container input {
    opacity: 0;
    width: 0;
    height: 0;
  }

  .switch-slider {
    position: absolute;
    cursor: pointer;
    top: 0;
    left: 0;
    right: 0;
    bottom: 0;
    background-color: #45475a;
    transition: 0.2s;
    border-radius: 20px;
  }

  .switch-slider:before {
    position: absolute;
    content: '';
    height: 14px;
    width: 14px;
    left: 3px;
    bottom: 3px;
    background-color: white;
    transition: 0.2s;
    border-radius: 50%;
  }

  .switch-container input:checked + .switch-slider {
    background-color: #a6e3a1;
  }

  .switch-container input:checked + .switch-slider:before {
    transform: translateX(16px);
  }

  .footer-settings-card {
    background: var(--color-surface, #1e1e2e);
    border: 1px solid var(--color-border, #313244);
    border-radius: 10px;
    padding: 14px;
  }

  .footer-settings-row {
    display: flex;
    justify-content: space-between;
    align-items: center;
    flex-wrap: wrap;
    gap: 12px;
  }
</style>
