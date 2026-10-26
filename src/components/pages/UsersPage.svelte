<script lang="ts">
  import PageHeader from '../PageHeader.svelte';
  import type { Config } from '../../lib/types';

  let {
    config,
    onRefresh,
    onToggleUser,
    onToggleScreenshot,
    onToggleNotifyAll,
  }: {
    config: Config;
    onRefresh: () => void;
    onToggleUser: (chatId: string, enabled: boolean) => void;
    onToggleScreenshot: (chatId: string, allowed: boolean) => void;
    onToggleNotifyAll: () => void;
  } = $props();
</script>

<PageHeader title="Пользователи">
  <button type="button" class="btn btn-secondary btn-sm" onclick={onRefresh}>Обновить</button>
</PageHeader>

<div class="users-page">
  <div class="card">
    <div class="card-header">
      <h2>Зарегистрированные ({(config.telegram?.registered_users || []).length})</h2>
    </div>
    <div class="card-body">
      <div class="form-group">
        <label class="checkbox-label">
          <input
            type="checkbox"
            checked={config.telegram?.notify_all_users ?? true}
            onchange={onToggleNotifyAll}
          />
          <span>Отправлять уведомления всем зарегистрированным</span>
        </label>
      </div>
      {#if (config.telegram?.registered_users || []).length === 0}
        <p class="empty-text">Нет пользователей — напишите боту /start</p>
      {:else}
        <ul class="user-list">
          {#each config.telegram.registered_users as user (user.chat_id)}
            {@const isAdmin = Boolean(config.telegram?.admin_chat_id && config.telegram.admin_chat_id === user.chat_id)}
            <li class="user-item">
              <div class="user-info">
                <span class="user-name">
                  {user.first_name || 'Unknown'}
                  {#if isAdmin}
                    <span class="admin-badge ok" style="margin-left: 6px;">👑 Админ</span>
                  {/if}
                </span>
                <span class="user-username">@{user.username || '-'}</span>
                <span class="user-id">ID: {user.chat_id}</span>
              </div>
              <div class="user-actions">
                <button
                  type="button"
                  class="btn btn-sm"
                  class:btn-success={user.enabled}
                  class:btn-danger={!user.enabled}
                  onclick={() => onToggleUser(user.chat_id, !user.enabled)}
                  title={user.enabled ? "Уведомления включены" : "Уведомления отключены"}
                >
                  {user.enabled ? '🔔 Уведомления: Вкл' : '🔕 Уведомления: Выкл'}
                </button>
                {#if isAdmin}
                  <button
                    type="button"
                    class="btn btn-sm btn-secondary"
                    disabled
                    title="Администратору всегда разрешены скриншоты"
                  >
                    📷 Скриншоты: Всегда
                  </button>
                {:else}
                  <button
                    type="button"
                    class="btn btn-sm"
                    class:btn-success={user.allow_screenshot}
                    class:btn-secondary={!user.allow_screenshot}
                    onclick={() => onToggleScreenshot(user.chat_id, !user.allow_screenshot)}
                    title={user.allow_screenshot ? "Разрешено делать скриншоты (/screen)" : "Запрещено делать скриншоты"}
                  >
                    📷 Скриншоты: {user.allow_screenshot ? 'Вкл' : 'Выкл'}
                  </button>
                {/if}
              </div>
            </li>
          {/each}
        </ul>
      {/if}
    </div>
  </div>
</div>
