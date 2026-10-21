<script lang="ts">
  import PageHeader from '../PageHeader.svelte';
  import type { Config } from '../../lib/types';

  let {
    config,
    newPatternName = $bindable(''),
    newPatternBytes = $bindable(''),
    newPatternDesc = $bindable(''),
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
  }: {
    config: Config;
    newPatternName?: string;
    newPatternBytes?: string;
    newPatternDesc?: string;
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
  } = $props();
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
            <li class="pattern-item">
              <div class="pattern-info">
                <span class="pattern-name">{pattern.name}</span>
                <span class="pattern-bytes">{pattern.bytes}</span>
                <span class="pattern-desc">{pattern.description}</span>
              </div>
              <button class="btn btn-danger btn-sm" onclick={() => onRemove(pattern.name)}>Удалить</button>
            </li>
          {/each}
        </ul>
      {/if}
    </div>
  </div>
</div>
