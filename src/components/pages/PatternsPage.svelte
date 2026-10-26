<script lang="ts">
  import PageHeader from '../PageHeader.svelte';
  import type { Config, Pattern } from '../../lib/types';

  let {
    config,
    newPatternName = $bindable(''),
    newPatternBytes = $bindable(''),
    newPatternDesc = $bindable(''),
    newPatternActionEnabled = $bindable(false),
    newPatternActionKey = $bindable('Z'),
    newPatternActionDelay = $bindable(1000),
    testHexInput = $bindable(''),
    testHexResult = '',
    nameFieldError = null,
    bytesFieldError = null,
    descFieldError = null,
    onTouchName,
    onTouchBytes,
    onTouchDesc,
    onAdd,
    onRemove,
    onReset,
    onExport,
    onImport,
    onTestHex,
    onSavePatternAction,
  }: {
    config: Config;
    newPatternName?: string;
    newPatternBytes?: string;
    newPatternDesc?: string;
    newPatternActionEnabled?: boolean;
    newPatternActionKey?: string;
    newPatternActionDelay?: number;
    testHexInput?: string;
    testHexResult?: string;
    nameFieldError?: string | null;
    bytesFieldError?: string | null;
    descFieldError?: string | null;
    onTouchName: () => void;
    onTouchBytes: () => void;
    onTouchDesc: () => void;
    onAdd: () => void;
    onRemove: (name: string) => void;
    onReset: () => void;
    onExport: () => void;
    onImport: (replace: boolean) => void;
    onTestHex: () => void;
    onSavePatternAction?: (pattern: Pattern) => void;
  } = $props();

  let openActionName = $state<string | null>(null);

  function toggleActionEditor(pattern: Pattern) {
    if (openActionName === pattern.name) {
      openActionName = null;
    } else {
      if (!pattern.action) {
        pattern.action = { enabled: false, key: 'Z', delay_ms: 1000 };
      }
      openActionName = pattern.name;
    }
  }

  function handleSaveAction(pattern: Pattern) {
    if (onSavePatternAction) {
      onSavePatternAction(pattern);
    }
    openActionName = null;
  }
</script>

<PageHeader title="Паттерны">
  <button type="button" class="btn btn-secondary btn-sm" onclick={onExport}>Экспорт</button>
  <button type="button" class="btn btn-secondary btn-sm" onclick={() => onImport(false)}>Импорт</button>
  <button type="button" class="btn btn-secondary btn-sm" onclick={() => onImport(true)}>Заменить</button>
  <button type="button" class="btn btn-danger btn-sm" onclick={onReset}>Сбросить паттерны</button>
</PageHeader>

<div class="patterns-page">
  <div class="card">
    <div class="card-header"><h2>Проверить hex</h2></div>
    <div class="card-body">
      <div class="form-group">
        <label for="test-hex">Payload hex</label>
        <textarea
          id="test-hex"
          class="hex-test-input"
          bind:value={testHexInput}
          placeholder="00 FE 06 03 2C 01 00 00"
          rows="3"
          spellcheck="false"
        ></textarea>
      </div>
      <div class="actions">
        <button type="button" class="btn btn-secondary" onclick={onTestHex}>Проверить</button>
      </div>
      {#if testHexResult}
        <p class="hex-test-result">{testHexResult}</p>
      {/if}
    </div>
  </div>

  <div class="card">
    <div class="card-header"><h2>Добавить паттерн</h2></div>
    <div class="card-body">
      <div class="add-pattern-form">
        <div class="form-row">
          <div class="form-group">
            <label for="pattern-name">Название (ID)</label>
            <input
              id="pattern-name"
              type="text"
              class:invalid={!!nameFieldError}
              bind:value={newPatternName}
              placeholder="STOP_FARM"
              autocomplete="off"
              spellcheck="false"
              onblur={onTouchName}
            />
            {#if nameFieldError}
              <p class="field-error">{nameFieldError}</p>
            {:else}
              <p class="field-hint">Латиница, цифры и _</p>
            {/if}
          </div>
          <div class="form-group">
            <label for="pattern-bytes">Hex байты</label>
            <input
              id="pattern-bytes"
              type="text"
              class:invalid={!!bytesFieldError}
              bind:value={newPatternBytes}
              placeholder="00 FE 06 03"
              autocomplete="off"
              spellcheck="false"
              onblur={onTouchBytes}
            />
            {#if bytesFieldError}
              <p class="field-error">{bytesFieldError}</p>
            {:else}
              <p class="field-hint">Пары hex, пробелы можно</p>
            {/if}
          </div>
        </div>
        <div class="form-group">
          <label for="pattern-desc">Описание события</label>
          <input
            id="pattern-desc"
            type="text"
            class:invalid={!!descFieldError}
            bind:value={newPatternDesc}
            placeholder="Автофарм остановлен"
            onblur={onTouchDesc}
          />
          {#if descFieldError}
            <p class="field-error">{descFieldError}</p>
          {:else}
            <p class="field-hint">Текст в уведомлениях</p>
          {/if}
        </div>

        <div class="form-group action-toggle-group">
          <label class="checkbox-label">
            <input type="checkbox" bind:checked={newPatternActionEnabled} />
            <span>⚡ Нажатие клавиши в окно игры при срабатывании</span>
          </label>
        </div>

        {#if newPatternActionEnabled}
          <div class="form-row action-config-row">
            <div class="form-group">
              <label for="action-key">Клавиша</label>
              <input
                id="action-key"
                type="text"
                class="action-input-key"
                bind:value={newPatternActionKey}
                placeholder="Z"
                maxlength="10"
              />
              <p class="field-hint">Z, 1..9, F1..F12, Space</p>
            </div>
            <div class="form-group">
              <label for="action-delay">Задержка перед нажатием (мс)</label>
              <input
                id="action-delay"
                type="number"
                class="action-input-delay"
                bind:value={newPatternActionDelay}
                min="0"
                step="100"
              />
              <p class="field-hint">1000 мс = 1 сек</p>
            </div>
          </div>
        {/if}

        <div class="actions">
          <button class="btn btn-success" onclick={onAdd}>Добавить паттерн</button>
        </div>
      </div>
    </div>
  </div>

  <div class="card">
    <div class="card-header">
      <h2>Список паттернов ({(config.target?.patterns || []).length})</h2>
    </div>
    <div class="card-body">
      {#if (config.target?.patterns || []).length === 0}
        <p class="empty-text">Нет паттернов</p>
      {:else}
        <ul class="pattern-list">
          {#each config.target.patterns as pattern}
            <li class="pattern-item-card">
              <div class="pattern-row">
                <div class="pattern-info">
                  <span class="pattern-name">{pattern.name}</span>
                  <span class="pattern-bytes">{pattern.bytes}</span>
                  <span class="pattern-desc">{pattern.description}</span>
                </div>
                <div class="pattern-actions-col">
                  <button
                    type="button"
                    class="pattern-action-badge"
                    class:active={pattern.action?.enabled}
                    onclick={() => toggleActionEditor(pattern)}
                    title="Нажмите для настройки нажатия клавиши"
                  >
                    {#if pattern.action?.enabled}
                      ⚡ [{pattern.action.key}] через {pattern.action.delay_ms}мс
                    {:else}
                      ⚡ Действие: Выкл
                    {/if}
                  </button>
                  <button
                    type="button"
                    class="btn btn-danger btn-sm"
                    onclick={() => onRemove(pattern.name)}
                  >
                    Удалить
                  </button>
                </div>
              </div>

              {#if openActionName === pattern.name}
                <div class="pattern-action-editor">
                  <div class="editor-header">
                    <strong>⚡ Настройка фонового действия для {pattern.name}</strong>
                  </div>
                  <div class="editor-body">
                    <label class="checkbox-label">
                      <input
                        type="checkbox"
                        checked={pattern.action?.enabled ?? false}
                        onchange={(e) => {
                          if (!pattern.action) {
                            pattern.action = { enabled: false, key: 'Z', delay_ms: 1000 };
                          }
                          pattern.action.enabled = e.currentTarget.checked;
                        }}
                      />
                      <span>Включить фоновое нажатие клавиши</span>
                    </label>

                    <div class="editor-inputs">
                      <div class="form-group inline">
                        <label for="edit-key-{pattern.name}">Клавиша:</label>
                        <input
                          id="edit-key-{pattern.name}"
                          type="text"
                          class="action-input-key"
                          value={pattern.action?.key ?? 'Z'}
                          placeholder="Z"
                          maxlength="10"
                          oninput={(e) => {
                            if (!pattern.action) {
                              pattern.action = { enabled: false, key: 'Z', delay_ms: 1000 };
                            }
                            pattern.action.key = e.currentTarget.value.trim().toUpperCase() || 'Z';
                          }}
                        />
                      </div>

                      <div class="form-group inline">
                        <label for="edit-delay-{pattern.name}">Задержка (мс):</label>
                        <input
                          id="edit-delay-{pattern.name}"
                          type="number"
                          class="action-input-delay"
                          value={pattern.action?.delay_ms ?? 1000}
                          min="0"
                          step="100"
                          oninput={(e) => {
                            if (!pattern.action) {
                              pattern.action = { enabled: false, key: 'Z', delay_ms: 1000 };
                            }
                            pattern.action.delay_ms = Math.max(0, Number(e.currentTarget.value) || 0);
                          }}
                        />
                      </div>

                      <button
                        type="button"
                        class="btn btn-primary btn-sm"
                        onclick={() => handleSaveAction(pattern)}
                      >
                        Сохранить
                      </button>
                    </div>
                  </div>
                </div>
              {/if}
            </li>
          {/each}
        </ul>
      {/if}
    </div>
  </div>
</div>
