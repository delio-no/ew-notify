<script lang="ts">
  import type { PageId } from '../lib/types';

  let {
    currentPage,
    isAdmin = false,
    onNavigate,
  }: {
    currentPage: PageId;
    isAdmin?: boolean;
    onNavigate: (page: PageId) => void;
  } = $props();

  const items: { id: PageId; label: string }[] = [
    { id: 'dashboard', label: 'Дашборд' },
    { id: 'settings', label: 'Настройки' },
    { id: 'patterns', label: 'Паттерны' },
    { id: 'notifications', label: 'Уведомления' },
    { id: 'users', label: 'Пользователи' },
    { id: 'console', label: 'Консоль' },
  ];
</script>

<aside class="sidebar">
  <div class="logo">
    <span class="logo-icon" aria-hidden="true">EW</span>
    <span class="logo-text">EW Notify</span>
  </div>
  <nav class="nav">
    {#each items as item}
      <button
        type="button"
        class="nav-item"
        class:active={currentPage === item.id}
        onclick={() => onNavigate(item.id)}
      >
        <span>{item.label}</span>
      </button>
    {/each}
  </nav>
  <div class="sidebar-footer">
    <div class="admin-badge" class:ok={isAdmin} class:warn={!isAdmin}>
      {isAdmin ? 'Admin' : 'Нет admin'}
    </div>
    <div class="version">v0.1.0</div>
  </div>
</aside>

<style>
  .logo-icon {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 32px;
    height: 32px;
    border-radius: 8px;
    background: var(--accent);
    color: #fff;
    font-size: 11px;
    font-weight: 800;
    letter-spacing: 0.02em;
  }
</style>
