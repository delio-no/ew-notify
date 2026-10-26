<script lang="ts">
  import PageHeader from '../PageHeader.svelte';
  import type { Config } from '../../lib/types';

  let {
    config = $bindable(),
    isBotRunning = false,
    isCaptureAuto = true,
    showBotToken = $bindable(false),
    onSave,
    onReset,
    onTestTelegram,
    onStartBot,
    onStopBot,
    onRestartBot,
  }: {
    config: Config;
    isBotRunning?: boolean;
    isCaptureAuto?: boolean;
    showBotToken?: boolean;
    onSave: () => void;
    onReset: () => void;
    onTestTelegram: () => void;
    onStartBot: () => void;
    onStopBot: () => void;
    onRestartBot: () => void;
  } = $props();
</script>

<PageHeader title="Настройки" />

<div class="settings-grid">
  <div class="card settings-card-telegram">
    <div class="card-header"><h2>Telegram</h2></div>
    <div class="card-body">
      <div class="form-group">
        <label>Bot Token</label>
        <div class="token-row">
          <input
            type={showBotToken ? 'text' : 'password'}
            bind:value={config.telegram.bot_token}
            placeholder="123456789:ABCdef…"
            autocomplete="new-password"
            spellcheck="false"
          />
          <button
            type="button"
            class="btn btn-secondary btn-sm"
            onclick={() => (showBotToken = !showBotToken)}
          >
            {showBotToken ? 'Скрыть' : 'Показать'}
          </button>
        </div>
        {#if isBotRunning && isTokenChanged}
          <div class="token-notice">
            <span class="notice-icon">⚠️</span>
            <span>Токен изменён. Бот сейчас работает со старым токеном.</span>
          </div>
        {/if}
      </div>
      <div class="form-group">
        <label>Admin chat ID (для /users)</label>
        <input
          type="text"
          bind:value={config.telegram.admin_chat_id}
          placeholder="из /myid"
          autocomplete="off"
          spellcheck="false"
        />
      </div>
      <div class="form-group">
        <label class="checkbox-label">
          <input type="checkbox" bind:checked={config.notifications.telegram_enabled} />
          <span>Включить Telegram уведомления</span>
        </label>
      </div>
      <div class="form-group">
        <label class="checkbox-label">
          <input type="checkbox" bind:checked={config.telegram.auto_start} />
          <span>Автозапуск бота при старте приложения</span>
        </label>
      </div>
      <div class="bot-status-row">
        <span class="bot-status-label">Бот:</span>
        <span class="bot-status-value" class:on={isBotRunning}>
          {isBotRunning ? 'запущен' : 'остановлен'}
        </span>
      </div>
      <div class="actions">
        <button class="btn btn-secondary" onclick={onTestTelegram}>Проверить токен</button>
        {#if isBotRunning}
          {#if isTokenChanged && onApplyAndRestartBot}
            <button class="btn btn-warning" onclick={onApplyAndRestartBot} title="Применить новый токен и перезапустить бота">
              Применить и перезапустить
            </button>
          {:else}
            <button class="btn btn-secondary" onclick={onRestartBot}>Перезапустить</button>
          {/if}
        {/if}
      </div>
      <div class="actions actions-primary">
        {#if isBotRunning}
          <button class="btn btn-danger btn-block" onclick={onStopBot}>Остановить бота</button>
        {:else}
          <button class="btn btn-success btn-block" onclick={onStartBot}>Запустить бота</button>
        {/if}
      </div>
    </div>
  </div>

  <div class="card settings-card-monitor">
    <div class="card-header"><h2>Мониторинг</h2></div>
    <div class="card-body">
      <div class="form-group">
        <label>Имя процесса</label>
        <input type="text" bind:value={config.target.process_name} placeholder="EndlessWar.exe" />
      </div>
      <div class="form-group">
        <label>Режим захвата</label>
        <div class="mode-row">
          <label class="checkbox-label">
            <input
              type="radio"
              name="capture_mode"
              checked={(config.target.capture_mode || 'auto') === 'auto'}
              onchange={() => (config.target.capture_mode = 'auto')}
            />
            <span>Авто (из процесса)</span>
          </label>
          <label class="checkbox-label">
            <input
              type="radio"
              name="capture_mode"
              checked={config.target.capture_mode === 'manual'}
              onchange={() => (config.target.capture_mode = 'manual')}
            />
            <span>Ручной (порт/IP)</span>
          </label>
        </div>
      </div>
      {#if isCaptureAuto}
        <p class="hint-text left">Remote IP/port берём из TCP-соединений процесса.</p>
      {:else}
        <div class="form-group">
          <label>TCP порт сервера (remote)</label>
          <input
            type="number"
            bind:value={config.target.tcp_port}
            min="1"
            max="65535"
            placeholder="7777"
          />
        </div>
        <div class="form-group">
          <label>IP сервера (опционально)</label>
          <input
            type="text"
            value={config.target.remote_ip ?? ''}
            oninput={(e) => {
              const v = (e.currentTarget as HTMLInputElement).value.trim();
              config.target.remote_ip = v || null;
            }}
            placeholder="пусто = любой"
            autocomplete="off"
            spellcheck="false"
          />
        </div>
      {/if}
      <div class="form-group">
        <label>Кулдаун (секунды)</label>
        <input type="number" bind:value={config.notifications.cooldown_seconds} min="0" />
      </div>
      <div class="form-group">
        <label>Burst dedup LAN+VPN (мс, 0 = выкл)</label>
        <input type="number" bind:value={config.notifications.burst_dedup_ms} min="0" step="100" />
      </div>
      <div class="form-group last">
        <label>Задержка скриншота после алерта (мс)</label>
        <input type="number" bind:value={config.notifications.screenshot_delay_ms} min="0" step="100" />
      </div>
    </div>
  </div>

  <div class="card settings-card-notify">
    <div class="card-header"><h2>Уведомления</h2></div>
    <div class="card-body">
      <div class="form-group last">
        <label class="checkbox-label">
          <input type="checkbox" bind:checked={config.notifications.tray_enabled} />
          <span>Системные уведомления</span>
        </label>
      </div>
    </div>
  </div>

  <div class="card settings-card-startup">
    <div class="card-header"><h2>Запуск и трей</h2></div>
    <div class="card-body">
      <div class="form-group">
        <label class="checkbox-label">
          <input type="checkbox" bind:checked={config.startup.auto_start_monitoring} />
          <span>Автозапуск мониторинга при старте приложения</span>
        </label>
      </div>
      <div class="form-group">
        <label class="checkbox-label">
          <input type="checkbox" bind:checked={config.startup.start_minimized_to_tray} />
          <span>При старте сразу сворачивать в трей</span>
        </label>
      </div>
      <div class="form-group last">
        <label class="checkbox-label">
          <input type="checkbox" bind:checked={config.startup.close_to_tray} />
          <span>Закрытие окна (×) сворачивает в трей</span>
        </label>
      </div>
    </div>
  </div>

  <div class="actions settings-footer">
    <button class="btn btn-primary" onclick={onSave}>Сохранить настройки</button>
    <button type="button" class="btn btn-danger" onclick={onReset}>Сбросить</button>
  </div>
</div>

<style>
  .token-notice {
    display: flex;
    align-items: center;
    gap: 6px;
    margin-top: 8px;
    font-size: 12px;
    color: var(--warn, #f0c674);
    background: rgba(240, 198, 116, 0.1);
    padding: 6px 10px;
    border-radius: 6px;
    border: 1px solid rgba(240, 198, 116, 0.25);
  }

  .btn-warning {
    background: #d97706;
    color: #fff;
  }
  .btn-warning:hover {
    background: #b45309;
  }
</style>
