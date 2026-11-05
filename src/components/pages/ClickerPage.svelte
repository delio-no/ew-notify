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
        stop_on_pattern: r.stop_on_pattern ?? false,
        stop_patterns: [...(r.stop_patterns || [])],
        start_on_pattern: r.start_on_pattern ?? false,
        start_patterns: [...(r.start_patterns || [])],
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
        stop_on_pattern: false,
        stop_patterns: [],
        start_on_pattern: false,
        start_patterns: [],
      },
    ];
  }

  let rules = $state<ClickerRule[]>([]);
  let notifyOnStop = $state(true);
  let notifyOnStart = $state(true);

  // Sync safely when clickerStatus config changes externally without reactive feedback loops
  $effect(() => {
    const externalRules = clickerStatus?.config?.rules;
    const externalNotifyStop = clickerStatus?.config?.notify_on_stop;
    const externalNotifyStart = clickerStatus?.config?.notify_on_start;
    untrack(() => {
      rules = parseInitialRules(externalRules);
      if (externalNotifyStop !== undefined) {
        notifyOnStop = externalNotifyStop;
      }
      if (externalNotifyStart !== undefined) {
        notifyOnStart = externalNotifyStart;
      }
    });
  });

  type WindowTabId = 'slot-1' | 'slot-2' | 'slot-3' | 'slot-4' | 'slot-5' | 'slot-6' | 'all-windows' | 'all';
  let activeWindowTab = $state<WindowTabId>('slot-1');

  function isSlotCaptured(slot: number): boolean {
    if (!windowFilterMode) return false;
    if (windowFilterMode.mode === 'Single') return windowFilterMode.data.slot === slot;
    if (windowFilterMode.mode === 'Multi') return windowFilterMode.data.some((d) => d.slot === slot);
    return false;
  }

  function getSlotWindowInfo(slot: number) {
    if (!windowFilterMode) return null;
    if (windowFilterMode.mode === 'Single') {
      return windowFilterMode.data.slot === slot ? windowFilterMode.data : null;
    }
    if (windowFilterMode.mode === 'Multi') {
      return windowFilterMode.data.find((d) => d.slot === slot) || null;
    }
    return null;
  }

  interface WindowTabOption {
    id: WindowTabId;
    label: string;
    icon: string;
    targetMatch: (t: ClickerTarget) => boolean;
    defaultTarget: ClickerTarget;
    description: string;
  }

  let windowTabs = $derived<WindowTabOption[]>([
    {
      id: 'slot-1',
      label: isSlotCaptured(1) ? 'Слот #1 (Захвачено)' : 'Слот #1',
      icon: isSlotCaptured(1) ? '🟢' : '⚪',
      targetMatch: (t) => (t?.type === 'Slot' && t.data === 1) || (!t || t.type === 'Captured'),
      defaultTarget: { type: 'Slot', data: 1 },
      description: isSlotCaptured(1)
        ? `Окно привязано (PID ${getSlotWindowInfo(1)?.pid}): клавиши отправляются строго в окно Слота #1`
        : 'Клавиши для окна Слота #1. Нажмите Ctrl+1 в окне игры, чтобы привязать его к этому слоту.',
    },
    {
      id: 'slot-2',
      label: isSlotCaptured(2) ? 'Слот #2 (Захвачено)' : 'Слот #2',
      icon: isSlotCaptured(2) ? '🟢' : '⚪',
      targetMatch: (t) => t?.type === 'Slot' && t.data === 2,
      defaultTarget: { type: 'Slot', data: 2 },
      description: isSlotCaptured(2)
        ? `Окно привязано (PID ${getSlotWindowInfo(2)?.pid}): клавиши отправляются строго в окно Слота #2`
        : 'Клавиши для окна Слота #2. Нажмите Ctrl+2 во втором окне игры, чтобы привязать его к этому слоту.',
    },
    {
      id: 'slot-3',
      label: isSlotCaptured(3) ? 'Слот #3 (Захвачено)' : 'Слот #3',
      icon: isSlotCaptured(3) ? '🟢' : '⚪',
      targetMatch: (t) => t?.type === 'Slot' && t.data === 3,
      defaultTarget: { type: 'Slot', data: 3 },
      description: isSlotCaptured(3)
        ? `Окно привязано (PID ${getSlotWindowInfo(3)?.pid}): клавиши отправляются строго в окно Слота #3`
        : 'Клавиши для окна Слота #3 (Ctrl+3 в окне игры)',
    },
    {
      id: 'slot-4',
      label: isSlotCaptured(4) ? 'Слот #4 (Захвачено)' : 'Слот #4',
      icon: isSlotCaptured(4) ? '🟢' : '⚪',
      targetMatch: (t) => t?.type === 'Slot' && t.data === 4,
      defaultTarget: { type: 'Slot', data: 4 },
      description: isSlotCaptured(4)
        ? `Окно привязано (PID ${getSlotWindowInfo(4)?.pid}): клавиши отправляются строго в окно Слота #4`
        : 'Клавиши для окна Слота #4 (Ctrl+4 в окне игры)',
    },
    {
      id: 'slot-5',
      label: isSlotCaptured(5) ? 'Слот #5 (Захвачено)' : 'Слот #5',
      icon: isSlotCaptured(5) ? '🟢' : '⚪',
      targetMatch: (t) => t?.type === 'Slot' && t.data === 5,
      defaultTarget: { type: 'Slot', data: 5 },
      description: isSlotCaptured(5)
        ? `Окно привязано (PID ${getSlotWindowInfo(5)?.pid}): клавиши отправляются строго в окно Слота #5`
        : 'Клавиши для окна Слота #5 (Ctrl+5 в окне игры)',
    },
    {
      id: 'slot-6',
      label: isSlotCaptured(6) ? 'Слот #6 (Захвачено)' : 'Слот #6',
      icon: isSlotCaptured(6) ? '🟢' : '⚪',
      targetMatch: (t) => t?.type === 'Slot' && t.data === 6,
      defaultTarget: { type: 'Slot', data: 6 },
      description: isSlotCaptured(6)
        ? `Окно привязано (PID ${getSlotWindowInfo(6)?.pid}): клавиши отправляются строго в окно Слота #6`
        : 'Клавиши для окна Слота #6 (Ctrl+6 в окне игры)',
    },
    {
      id: 'all-windows',
      label: 'Все окна',
      icon: '🌐',
      targetMatch: (t) => t?.type === 'All',
      defaultTarget: { type: 'All' },
      description: 'Рассылка клавиши параллельно во все запущенные окна игры EndlessWar.exe',
    },
    {
      id: 'all',
      label: 'Все правила',
      icon: '📑',
      targetMatch: () => true,
      defaultTarget: { type: 'Slot', data: 1 },
      description: 'Общий сводный список всех настроенных правил для всех окон',
    },
  ]);

  function getTabRuleCount(tab: WindowTabOption): number {
    return (rules || []).filter((r) => tab.targetMatch(r.target)).length;
  }

  let activeTabOption = $derived(
    windowTabs.find((t) => t.id === activeWindowTab) || windowTabs[0]
  );

  let visibleRules = $derived(
    (rules || []).filter((r) => {
      const tab = windowTabs.find((t) => t.id === activeWindowTab);
      return tab ? tab.targetMatch(r.target) : true;
    })
  );

  const quickKeys = ['1', '2', '3', '4', 'F1', 'F2', 'F3', 'F4', 'Z', 'Space'];
  const quickIntervals = [200, 500, 1000, 2000, 3000, 5000];

  function addRule() {
    const nextNum = rules.length + 1;
    const currentTab = windowTabs.find((t) => t.id === activeWindowTab);
    const target = currentTab ? { ...currentTab.defaultTarget } : { type: 'Slot' as const, data: 1 };
    const newRule: ClickerRule = {
      id: `rule-${Date.now()}-${Math.random().toString(36).slice(2, 6)}`,
      name: `Клавиша ${nextNum}`,
      enabled: true,
      key: nextNum <= 9 ? `${nextNum}` : '1',
      interval_ms: 1000,
      target,
      stop_on_pattern: false,
      stop_patterns: [],
      start_on_pattern: false,
      start_patterns: [],
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
          target: { type: 'Slot', data: 1 },
          stop_on_pattern: false,
          stop_patterns: [],
          start_on_pattern: false,
          start_patterns: [],
        },
      ];
      return;
    }
    rules = rules.filter((r) => r.id !== id);
  }

  function toggleRuleStopPattern(rule: ClickerRule, patName: string) {
    if (!rule.stop_patterns) rule.stop_patterns = [];
    if (rule.stop_patterns.includes(patName)) {
      rule.stop_patterns = rule.stop_patterns.filter((p) => p !== patName);
    } else {
      rule.stop_patterns = [...rule.stop_patterns, patName];
    }
  }

  function toggleRuleStartPattern(rule: ClickerRule, patName: string) {
    if (!rule.start_patterns) rule.start_patterns = [];
    if (rule.start_patterns.includes(patName)) {
      rule.start_patterns = rule.start_patterns.filter((p) => p !== patName);
    } else {
      rule.start_patterns = [...rule.start_patterns, patName];
    }
  }

  function handleSave() {
    const cleanedRules = rules.map((r) => {
      let target = r.target;
      if (!target || target.type === 'Captured') {
        target = { type: 'Slot', data: 1 };
      }
      return {
        ...r,
        target,
        key: r.key.trim().toUpperCase() || '1',
        interval_ms: Math.max(50, Number(r.interval_ms) || 1000),
        stop_patterns: [...(r.stop_patterns || [])],
        start_patterns: [...(r.start_patterns || [])],
      };
    });

    onSave({
      rules: cleanedRules,
      notify_on_stop: notifyOnStop,
      notify_on_start: notifyOnStart,
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
        <span class="banner-icon">🎮</span>
        <span>
          Привязано окно: <strong>Слот #{windowFilterMode.data.slot} [PID {windowFilterMode.data.pid}]</strong> — {windowFilterMode.data.title || 'EndlessWar'}
        </span>
      </div>
    {:else if windowFilterMode?.mode === 'Multi'}
      <div class="target-window-banner captured">
        <span class="banner-icon">🎮</span>
        <span>
          Привязано окон: <strong>{windowFilterMode.data.length}</strong> ({windowFilterMode.data.map((d) => `Слот #${d.slot} [PID ${d.pid}]`).join(', ')}) — клавиши настраиваются независимо по вкладкам слотов
        </span>
      </div>
    {:else}
      <div class="target-window-banner warning">
        <span class="banner-icon">ℹ️</span>
        <span>
          Окна игры не захвачены. Нажмите <strong>Ctrl + 1</strong> в первом окне игры и <strong>Ctrl + 2</strong> во втором окне, чтобы привязать их к слотам.
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

  <!-- Window Navigation Tabs -->
  <div class="window-tabs-container">
    <div class="window-tabs-scroll">
      {#each windowTabs as tab (tab.id)}
        {@const count = getTabRuleCount(tab)}
        <button
          type="button"
          class="window-tab-btn"
          class:active={activeWindowTab === tab.id}
          onclick={() => (activeWindowTab = tab.id)}
        >
          <span class="tab-icon">{tab.icon}</span>
          <span class="tab-label">{tab.label}</span>
          {#if count > 0}
            <span class="tab-badge">{count}</span>
          {/if}
        </button>
      {/each}
    </div>

    <div class="tab-context-banner">
      <div class="tab-context-left">
        <span class="tab-info-icon">{activeTabOption.icon}</span>
        <span class="tab-info-desc">{activeTabOption.description}</span>
      </div>
      <span class="tab-rule-stat">
        {visibleRules.length === rules.length
          ? `Всего правил: ${rules.length}`
          : `Показано ${visibleRules.length} из ${rules.length} правил`}
      </span>
    </div>
  </div>

  <!-- Rules List Header -->
  <div class="rules-section-header">
    <div class="rules-header-title">
      <h2>Клавиши: «{activeTabOption.label}» ({visibleRules.length})</h2>
      <p class="section-desc">Независимый интервал, привязка к окну и триггеры запуска/остановки по паттернам</p>
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

  <!-- Rules Cards List / Empty State -->
  {#if visibleRules.length === 0}
    <div class="empty-rules-card card">
      <div class="empty-icon">{activeTabOption.icon}</div>
      <div class="empty-title">Нет клавиш для вкладки «{activeTabOption.label}»</div>
      <p class="empty-desc">
        Нажмите кнопку ниже, чтобы создать новое действие. Оно сразу получит привязку <strong>{activeTabOption.label}</strong>.
      </p>
      <button type="button" class="btn btn-primary" onclick={addRule}>
        + Добавить клавишу для «{activeTabOption.label}»
      </button>
    </div>
  {:else}
    <div class="rules-list">
      {#each visibleRules as rule (rule.id)}
        {@const globalIdx = rules.findIndex((r) => r.id === rule.id) + 1}
        <div class="card rule-card" class:disabled={!rule.enabled}>
          <div class="rule-card-header">
            <div class="rule-header-left">
              <label class="switch-container" title="Включить / выключить эту клавишу">
                <input type="checkbox" bind:checked={rule.enabled} />
                <span class="switch-slider"></span>
              </label>

              <span class="rule-index">#{globalIdx > 0 ? globalIdx : 1}</span>

              <input
                type="text"
                class="rule-name-input"
                bind:value={rule.name}
                placeholder="Название действия (например: Скилл 1)"
              />
            </div>

            <div class="rule-header-right">
              {#if rule.target?.type === 'Slot'}
                <span class="rule-slot-badge">Слот #{rule.target.data}</span>
              {:else if rule.target?.type === 'All'}
                <span class="rule-slot-badge all">Все окна</span>
              {:else}
                <span class="rule-slot-badge">Слот #1</span>
              {/if}

              <span class="rule-key-preview">{rule.key || '?'}</span>
              <span class="rule-interval-preview">{rule.interval_ms} мс</span>

              {#if onTestKey}
                <button
                  type="button"
                  class="btn btn-secondary btn-sm"
                  onclick={() => onTestKey(rule.key, rule.target)}
                  title="Отправить эту клавишу прямо сейчас в окно этого слота"
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
              <!-- Key Field (Large prominent input) -->
              <div class="form-group key-form-group">
                <span class="field-title">
                  Клавиша
                  <span class="hint-text">(нажмите и задайте любую клавишу F1-F12, 1-9...)</span>
                </span>
                <div class="key-recorder-box">
                  <KeyRecorder bind:value={rule.key} size="large" />
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
            </div>

            <!-- Start on Pattern Trigger -->
            <div class="rule-pattern-section start-section">
              <label class="checkbox-label">
                <input type="checkbox" bind:checked={rule.start_on_pattern} />
                <span class="pattern-checkbox-text">
                  <span class="pattern-action-icon">▶</span>
                  <strong>Включать</strong> эту клавишу при обнаружении сетевого паттерна
                </span>
              </label>

              {#if rule.start_on_pattern}
                <div class="rule-pattern-chips">
                  <span class="chips-label">Запускать при:</span>
                  <button
                    type="button"
                    class="pattern-chip"
                    class:active-start={!rule.start_patterns || rule.start_patterns.length === 0}
                    onclick={() => (rule.start_patterns = [])}
                  >
                    ⚡ Любой паттерн
                  </button>
                  {#each patterns as pat}
                    <button
                      type="button"
                      class="pattern-chip"
                      class:active-start={(rule.start_patterns || []).includes(pat.name)}
                      onclick={() => toggleRuleStartPattern(rule, pat.name)}
                    >
                      {pat.name}
                    </button>
                  {/each}
                </div>
                <p class="pattern-hint">
                  {#if !rule.start_patterns || rule.start_patterns.length === 0}
                    Клавиша автоматически начнет нажиматься при срабатывании <em>любого</em> сетевого паттерна.
                  {:else}
                    Клавиша запустится только при срабатывании: <strong>{rule.start_patterns.join(', ')}</strong>.
                  {/if}
                </p>
              {/if}
            </div>

            <!-- Stop on Pattern Trigger -->
            <div class="rule-pattern-section stop-section">
              <label class="checkbox-label">
                <input type="checkbox" bind:checked={rule.stop_on_pattern} />
                <span class="pattern-checkbox-text">
                  <span class="pattern-action-icon">⏹</span>
                  <strong>Останавливать</strong> эту клавишу при обнаружении сетевого паттерна
                </span>
              </label>

              {#if rule.stop_on_pattern}
                <div class="rule-pattern-chips">
                  <span class="chips-label">Останавливать при:</span>
                  <button
                    type="button"
                    class="pattern-chip"
                    class:active-stop={!rule.stop_patterns || rule.stop_patterns.length === 0}
                    onclick={() => (rule.stop_patterns = [])}
                  >
                    ⚡ Любой паттерн
                  </button>
                  {#each patterns as pat}
                    <button
                      type="button"
                      class="pattern-chip"
                      class:active-stop={(rule.stop_patterns || []).includes(pat.name)}
                      onclick={() => toggleRuleStopPattern(rule, pat.name)}
                    >
                      {pat.name}
                    </button>
                  {/each}
                </div>
                <p class="pattern-hint">
                  {#if !rule.stop_patterns || rule.stop_patterns.length === 0}
                    Клавиша автоматически остановится при срабатывании <em>любого</em> сетевого паттерна.
                  {:else}
                    Клавиша остановится только при срабатывании: <strong>{rule.stop_patterns.join(', ')}</strong>.
                  {/if}
                </p>
              {/if}
            </div>
          </div>
        </div>
      {/each}
    </div>
  {/if}

  <!-- General Settings / Notification Card -->
  <div class="card footer-settings-card">
    <div class="footer-settings-title">Общие настройки автокликера</div>
    <div class="footer-settings-grid">
      <label class="checkbox-label">
        <input type="checkbox" bind:checked={notifyOnStart} />
        <span>Показывать системное уведомление при автозапуске клавиш по паттерну</span>
      </label>

      <label class="checkbox-label">
        <input type="checkbox" bind:checked={notifyOnStop} />
        <span>Показывать системное уведомление при автоостановке клавиш по паттерну</span>
      </label>
    </div>

    <div class="footer-save-row">
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
    grid-template-columns: 1fr 1fr;
    gap: 16px;
  }

  @media (max-width: 768px) {
    .rule-fields-grid {
      grid-template-columns: 1fr;
    }
  }

  .rule-slot-badge {
    font-size: 0.78rem;
    font-weight: 600;
    padding: 3px 8px;
    border-radius: 6px;
    background: rgba(34, 197, 94, 0.15);
    border: 1px solid rgba(34, 197, 94, 0.35);
    color: #4ade80;
    white-space: nowrap;
  }

  .rule-slot-badge.all {
    background: rgba(59, 130, 246, 0.15);
    border-color: rgba(59, 130, 246, 0.35);
    color: #60a5fa;
  }

  .field-title {
    font-size: 0.85rem;
    font-weight: 600;
    color: var(--text-muted, #a6adc8);
    display: flex;
    align-items: center;
    gap: 6px;
    margin-bottom: 6px;
  }

  .key-recorder-box {
    display: flex;
    align-items: center;
  }

  .window-tabs-container {
    display: flex;
    flex-direction: column;
    gap: 8px;
    background: var(--color-surface, #1e1e2e);
    border: 1px solid var(--color-border, #313244);
    border-radius: 12px;
    padding: 10px 12px;
  }

  .window-tabs-scroll {
    display: flex;
    gap: 6px;
    overflow-x: auto;
    padding-bottom: 4px;
    scrollbar-width: thin;
  }

  .window-tab-btn {
    display: inline-flex;
    align-items: center;
    gap: 7px;
    padding: 8px 14px;
    background: rgba(255, 255, 255, 0.04);
    border: 1px solid var(--color-border, #313244);
    border-radius: 8px;
    color: #a6adc8;
    font-size: 0.88rem;
    font-weight: 600;
    cursor: pointer;
    white-space: nowrap;
    transition: all 0.15s ease;
  }

  .window-tab-btn:hover {
    background: rgba(255, 255, 255, 0.08);
    color: #cdd6f4;
    border-color: rgba(137, 180, 250, 0.4);
  }

  .window-tab-btn.active {
    background: rgba(137, 180, 250, 0.18);
    border-color: #89b4fa;
    color: #89b4fa;
    box-shadow: 0 0 12px rgba(137, 180, 250, 0.2);
  }

  .tab-icon {
    font-size: 1rem;
  }

  .tab-badge {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    min-width: 18px;
    height: 18px;
    padding: 0 5px;
    border-radius: 9px;
    background: rgba(255, 255, 255, 0.12);
    color: #cdd6f4;
    font-size: 0.72rem;
    font-weight: 700;
  }

  .window-tab-btn.active .tab-badge {
    background: #89b4fa;
    color: #11111b;
  }

  .tab-context-banner {
    display: flex;
    align-items: center;
    justify-content: space-between;
    flex-wrap: wrap;
    gap: 10px;
    padding: 8px 12px;
    background: rgba(0, 0, 0, 0.2);
    border-radius: 6px;
    font-size: 0.83rem;
  }

  .tab-context-left {
    display: flex;
    align-items: center;
    gap: 8px;
    flex: 1;
    min-width: 200px;
  }

  .tab-info-icon {
    font-size: 1rem;
  }

  .tab-info-desc {
    color: #bac2de;
  }

  .tab-rule-stat {
    color: #a6adc8;
    font-size: 0.8rem;
    font-weight: 600;
  }

  .empty-rules-card {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    padding: 3rem 1.5rem;
    text-align: center;
    gap: 12px;
    background: var(--color-surface, #1e1e2e);
    border: 1px dashed var(--color-border, #313244);
    border-radius: 12px;
  }

  .empty-icon {
    font-size: 2.5rem;
    opacity: 0.7;
  }

  .empty-title {
    font-size: 1.15rem;
    font-weight: 700;
    color: #cdd6f4;
  }

  .empty-desc {
    font-size: 0.88rem;
    color: #a6adc8;
    max-width: 480px;
    margin: 0;
  }

  .rule-pattern-section {
    border-radius: 8px;
    padding: 10px 14px;
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  .rule-pattern-section.start-section {
    background: rgba(166, 227, 161, 0.05);
    border: 1px solid rgba(166, 227, 161, 0.2);
  }

  .rule-pattern-section.stop-section {
    background: rgba(243, 139, 168, 0.05);
    border: 1px solid rgba(243, 139, 168, 0.2);
  }

  .pattern-checkbox-text {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    color: #cdd6f4;
  }

  .pattern-action-icon {
    font-size: 0.85rem;
  }

  .start-section .pattern-action-icon {
    color: #a6e3a1;
  }

  .stop-section .pattern-action-icon {
    color: #f38ba8;
  }

  .pattern-chip.active-start {
    background: rgba(166, 227, 161, 0.2);
    border-color: #a6e3a1;
    color: #a6e3a1;
    font-weight: 700;
  }

  .pattern-chip.active-stop {
    background: rgba(243, 139, 168, 0.2);
    border-color: #f38ba8;
    color: #f38ba8;
    font-weight: 700;
  }

  .pattern-hint {
    font-size: 0.8rem;
    color: #a6adc8;
    margin: 2px 0 0 0;
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
    padding: 16px;
    display: flex;
    flex-direction: column;
    gap: 14px;
  }

  .footer-settings-title {
    font-weight: 700;
    font-size: 0.95rem;
    color: #cdd6f4;
  }

  .footer-settings-grid {
    display: flex;
    flex-direction: column;
    gap: 10px;
  }

  .footer-save-row {
    display: flex;
    justify-content: flex-end;
    padding-top: 8px;
    border-top: 1px solid rgba(255, 255, 255, 0.05);
  }
</style>
