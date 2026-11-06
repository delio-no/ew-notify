<script lang="ts">
  import { onMount } from 'svelte';
  import PageHeader from '../PageHeader.svelte';
  import { api } from '../../lib/api';
  import type { Config, UpdateInfo } from '../../lib/types';

  let {
    config = $bindable(),
    isBotRunning = false,
    isCaptureAuto = true,
    isTokenChanged = false,
    showBotToken = $bindable(false),
    onSave,
    onReset,
    onTestTelegram,
    onTestSystemNotification,
    onStartBot,
    onStopBot,
    onRestartBot,
    onApplyAndRestartBot,
  }: {
    config: Config;
    isBotRunning?: boolean;
    isCaptureAuto?: boolean;
    isTokenChanged?: boolean;
    showBotToken?: boolean;
    onSave: () => void;
    onReset: () => void;
    onTestTelegram: () => void;
    onTestSystemNotification?: () => void;
    onStartBot: () => void;
    onStopBot: () => void;
    onRestartBot: () => void;
    onApplyAndRestartBot?: () => void;
  } = $props();

  let currentAppVersion = $state('v0.3.1');
  let updateStatus = $state<'idle' | 'checking' | 'up-to-date' | 'available' | 'downloading' | 'error'>('idle');
  let updateInfo = $state<UpdateInfo | null>(null);
  let updateError = $state<string | null>(null);
  let lastCheckedTime = $state<string | null>(null);

  onMount(async () => {
    try {
      currentAppVersion = await api.getAppVersion();
    } catch {
      // ignore
    }
  });

  async function checkUpdates() {
    updateStatus = 'checking';
    updateError = null;
    try {
      const info = await api.checkForUpdates();
      updateInfo = info;
      lastCheckedTime = new Date().toLocaleTimeString();
      if (info.has_update) {
        updateStatus = 'available';
      } else {
        updateStatus = 'up-to-date';
      }
    } catch (e) {
      updateStatus = 'error';
      updateError = String(e);
      lastCheckedTime = new Date().toLocaleTimeString();
    }
  }

  async function installUpdate() {
    if (!updateInfo || !updateInfo.download_url) return;
    updateStatus = 'downloading';
    updateError = null;
    try {
      await api.installUpdate(updateInfo.download_url, updateInfo.asset_name);
    } catch (e) {
      updateStatus = 'error';
      updateError = `Ошибка установки обновления: ${e}`;
    }
  }
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
      <div class="form-group">
        <label class="checkbox-label">
          <input type="checkbox" bind:checked={config.notifications.tray_enabled} />
          <span>Системные уведомления Windows</span>
        </label>
        {#if onTestSystemNotification}
          <div style="margin-top: 8px;">
            <button type="button" class="btn btn-secondary btn-xs" onclick={onTestSystemNotification}>
              Проверить уведомление Windows
            </button>
          </div>
        {/if}
      </div>
      <div class="form-group">
        <label class="checkbox-label">
          <input type="checkbox" bind:checked={config.notifications.screenshot_on_death} />
          <span>Отправлять скриншот при гибели персонажа (DEATH)</span>
        </label>
      </div>
      <div class="form-group last">
        <label class="checkbox-label">
          <input type="checkbox" bind:checked={config.notifications.actions_enabled} />
          <span>Разрешить фоновую отправку клавиш в игру</span>
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

  <div class="card settings-card-updater">
    <div class="card-header update-card-header">
      <h2>Обновление программы</h2>
      <span class="version-badge">{currentAppVersion || 'v0.3.1'}</span>
    </div>
    <div class="card-body">
      <div class="form-group">
        <label class="checkbox-label">
          <input
            type="checkbox"
            checked={config.updater ? config.updater.check_on_startup : true}
            onchange={(e) => {
              if (!config.updater) config.updater = { check_on_startup: true, ignored_version: null };
              config.updater.check_on_startup = (e.target as HTMLInputElement).checked;
            }}
          />
          <span>Проверять наличие обновлений при запуске приложения</span>
        </label>
      </div>

      <div class="update-actions">
        <button
          type="button"
          class="btn btn-secondary btn-sm"
          disabled={updateStatus === 'checking' || updateStatus === 'downloading'}
          onclick={checkUpdates}
        >
          {updateStatus === 'checking' ? 'Проверка...' : 'Проверить обновления'}
        </button>

        {#if lastCheckedTime}
          <span class="last-checked">Проверено: {lastCheckedTime}</span>
        {/if}
      </div>

      {#if updateStatus === 'up-to-date'}
        <div class="update-notice success">
          <span class="notice-icon">✓</span>
          <span>У вас установлена самая актуальная версия программы.</span>
        </div>
      {:else if updateStatus === 'error'}
        <div class="update-notice error">
          <span class="notice-icon">⚠</span>
          <span>{updateError || 'Ошибка проверки обновлений'}</span>
        </div>
      {:else if updateStatus === 'available' && updateInfo}
        <div class="update-card-details">
          <div class="update-header">
            <div>
              <div class="update-tag">Доступна версия {updateInfo.latest_version}</div>
              {#if updateInfo.published_at}
                <div class="update-date">Опубликовано: {new Date(updateInfo.published_at).toLocaleDateString()}</div>
              {/if}
            </div>
            <button
              type="button"
              class="btn btn-primary btn-sm btn-update"
              disabled={updateStatus === 'downloading'}
              onclick={installUpdate}
            >
              Скачать и обновить
            </button>
          </div>

          {#if updateInfo.release_notes}
            <div class="release-notes-box">
              <div class="release-notes-title">Что нового в обновлении:</div>
              <pre class="release-notes-content">{updateInfo.release_notes}</pre>
            </div>
          {/if}
        </div>
      {:else if updateStatus === 'downloading'}
        <div class="update-notice downloading">
          <div class="spinner"></div>
          <span>Скачивание обновления и замена файлов... Приложение автоматически перезапустится.</span>
        </div>
      {/if}
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

  .update-card-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
  }

  .version-badge {
    font-size: 11px;
    font-weight: 700;
    color: var(--accent, #60a5fa);
    background: rgba(96, 165, 250, 0.12);
    padding: 2px 8px;
    border-radius: 999px;
    border: 1px solid rgba(96, 165, 250, 0.25);
  }

  .update-actions {
    display: flex;
    align-items: center;
    gap: 12px;
    margin-top: 10px;
    margin-bottom: 8px;
  }

  .last-checked {
    font-size: 11px;
    color: var(--text-muted, #888);
  }

  .update-notice {
    display: flex;
    align-items: center;
    gap: 8px;
    margin-top: 12px;
    padding: 10px 14px;
    border-radius: 6px;
    font-size: 13px;
  }

  .update-notice.success {
    background: rgba(34, 197, 94, 0.1);
    color: #4ade80;
    border: 1px solid rgba(34, 197, 94, 0.25);
  }

  .update-notice.error {
    background: rgba(239, 68, 68, 0.1);
    color: #f87171;
    border: 1px solid rgba(239, 68, 68, 0.25);
  }

  .update-notice.downloading {
    background: rgba(96, 165, 250, 0.1);
    color: #93c5fd;
    border: 1px solid rgba(96, 165, 250, 0.25);
  }

  .update-card-details {
    margin-top: 12px;
    padding: 14px;
    background: rgba(255, 255, 255, 0.03);
    border: 1px solid rgba(96, 165, 250, 0.3);
    border-radius: 8px;
  }

  .update-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    margin-bottom: 12px;
  }

  .update-tag {
    font-size: 15px;
    font-weight: 700;
    color: var(--accent, #60a5fa);
  }

  .update-date {
    font-size: 11px;
    color: var(--text-muted, #888);
    margin-top: 2px;
  }

  .btn-update {
    background: #2563eb;
    color: #fff;
    font-weight: 600;
  }
  .btn-update:hover {
    background: #1d4ed8;
  }

  .release-notes-box {
    margin-top: 10px;
    padding: 10px 12px;
    background: rgba(0, 0, 0, 0.25);
    border-radius: 6px;
    border: 1px solid rgba(255, 255, 255, 0.06);
  }

  .release-notes-title {
    font-size: 12px;
    font-weight: 600;
    color: var(--text-muted, #aaa);
    margin-bottom: 6px;
  }

  .release-notes-content {
    font-family: inherit;
    font-size: 12px;
    color: var(--text-normal, #ddd);
    white-space: pre-wrap;
    word-break: break-word;
    margin: 0;
    max-height: 160px;
    overflow-y: auto;
  }

  .spinner {
    width: 16px;
    height: 16px;
    border: 2px solid rgba(255, 255, 255, 0.2);
    border-top-color: #60a5fa;
    border-radius: 50%;
    animation: spin 0.8s linear infinite;
    flex-shrink: 0;
  }

  @keyframes spin {
    to {
      transform: rotate(360deg);
    }
  }
</style>
