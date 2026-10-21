<script lang="ts">
  import type { PageId } from '../lib/types';
  import { api } from '../lib/api';

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

  const GITHUB_REPO_URL = 'https://github.com/delio-no/ew-notify';
  const GITHUB_RELEASES_URL = 'https://github.com/delio-no/ew-notify/releases';

  function openRepo() {
    api.openUrl(GITHUB_REPO_URL).catch((e) => console.error(e));
  }

  function openReleases() {
    api.openUrl(GITHUB_RELEASES_URL).catch((e) => console.error(e));
  }
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
    <button
      type="button"
      class="github-link-btn"
      onclick={openRepo}
      title="Открыть репозиторий на GitHub"
    >
      <svg class="github-icon" viewBox="0 0 16 16" width="14" height="14" fill="currentColor" aria-hidden="true">
        <path d="M8 0C3.58 0 0 3.58 0 8c0 3.54 2.29 6.53 5.47 7.59.4.07.55-.17.55-.38 0-.19-.01-.82-.01-1.49-2.01.37-2.53-.49-2.69-.94-.09-.23-.48-.94-.82-1.13-.28-.15-.68-.52-.01-.53.63-.01 1.08.58 1.23.82.72 1.21 1.87.87 2.33.66.07-.52.28-.87.51-1.07-1.78-.2-3.64-.89-3.64-3.95 0-.87.31-1.59.82-2.15-.08-.2-.36-1.02.08-2.12 0 0 .67-.21 2.2.82.64-.18 1.32-.27 2-.27.68 0 1.36.09 2 .27 1.53-1.04 2.2-.82 2.2-.82.44 1.1.16 1.92.08 2.12.51.56.82 1.27.82 2.15 0 3.07-1.87 3.75-3.65 3.95.29.25.54.73.54 1.48 0 1.07-.01 1.93-.01 2.2 0 .21.15.46.55.38A8.013 8.013 0 0016 8c0-4.42-3.58-8-8-8z"></path>
      </svg>
      <span>GitHub</span>
    </button>
    <div class="admin-badge" class:ok={isAdmin} class:warn={!isAdmin}>
      {isAdmin ? 'Admin' : 'Нет admin'}
    </div>
    <button
      type="button"
      class="version-link"
      onclick={openReleases}
      title="Посмотреть релиз на GitHub"
    >
      v0.1.0
    </button>
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

  .github-link-btn {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    padding: 6px 12px;
    background: rgba(255, 255, 255, 0.05);
    border: 1px solid var(--border);
    border-radius: 6px;
    color: var(--text-dim, #a0a0a0);
    font-size: 12px;
    font-weight: 500;
    cursor: pointer;
    transition: all 0.2s ease;
    width: 100%;
    justify-content: center;
  }

  .github-link-btn:hover {
    background: rgba(255, 255, 255, 0.1);
    border-color: var(--accent, #6366f1);
    color: #fff;
  }

  .github-icon {
    flex-shrink: 0;
  }

  .version-link {
    background: none;
    border: none;
    color: var(--text-dim, #888);
    font-size: 12px;
    cursor: pointer;
    text-decoration: underline;
    text-underline-offset: 3px;
    padding: 2px 4px;
    transition: color 0.15s ease;
  }

  .version-link:hover {
    color: var(--accent, #6366f1);
  }
</style>
