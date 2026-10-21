<script lang="ts">
  import PageHeader from '../PageHeader.svelte';
  import type { StoredNotification } from '../../lib/types';

  let { notifications = [] }: { notifications?: StoredNotification[] } = $props();
</script>

<PageHeader title="История уведомлений" />

<div class="notifications-page">
  <div class="card">
    <div class="card-header">
      <h2>Все уведомления ({notifications.length})</h2>
    </div>
    <div class="card-body">
      {#if notifications.length === 0}
        <p class="empty-text">Нет уведомлений</p>
      {:else}
        <ul class="full-notification-list">
          {#each notifications as notif}
            <li class="notification-item">
              <div class="notif-content">
                <span class="notif-time">{notif.time}</span>
                <span class="notif-title">{notif.title}</span>
                <span class="notif-body">{notif.body}</span>
              </div>
            </li>
          {/each}
        </ul>
      {/if}
    </div>
  </div>
</div>
