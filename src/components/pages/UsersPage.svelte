<script lang="ts">
  import PageHeader from '../PageHeader.svelte';
  import type { Config } from '../../lib/types';

  let {
    config,
    onRefresh,
    onToggleUser,
    onToggleNotifyAll,
  }: {
    config: Config;
    onRefresh: () => void;
    onToggleUser: (chatId: string, enabled: boolean) => void;
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
            <li class="user-item">
              <div class="user-info">
                <span class="user-name">{user.first_name || 'Unknown'}</span>
                <span class="user-username">@{user.username || '-'}</span>
                <span class="user-id">ID: {user.chat_id}</span>
              </div>
              <div class="user-actions">
                <button
                  class="btn btn-sm"
                  class:btn-success={user.enabled}
                  class:btn-danger={!user.enabled}
                  onclick={() => onToggleUser(user.chat_id, !user.enabled)}
                >
                  {user.enabled ? 'Включён' : 'Отключён'}
                </button>
              </div>
            </li>
          {/each}
        </ul>
      {/if}
    </div>
  </div>
</div>
