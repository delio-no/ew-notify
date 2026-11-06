<script lang="ts">
  import { onMount } from 'svelte';
  import type { PageId } from '../lib/types';
  import { api } from '../lib/api';

  let {
    currentPage,
    isAdmin = false,
    hasUpdate = false,
    updateVersion = '',
    isUpdating = false,
    onNavigate,
    onDirectUpdate,
  }: {
    currentPage: PageId;
    isAdmin?: boolean;
    hasUpdate?: boolean;
    updateVersion?: string;
    isUpdating?: boolean;
    onNavigate: (page: PageId) => void;
    onDirectUpdate?: () => void;
  } = $props();

  let appVersion = $state('v0.3.15');

  onMount(async () => {
    try {
      const v = await api.getAppVersion();
      if (v) {
        appVersion = v.startsWith('v') ? v : `v${v}`;
      }
    } catch {
      // fallback to default
    }
  });

  const items: { id: PageId; label: string }[] = [
    { id: 'dashboard', label: 'Дашборд' },
    { id: 'settings', label: 'Настройки' },
    { id: 'patterns', label: 'Паттерны' },
    { id: 'clicker', label: 'Кликер' },
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
        {#if item.id === 'settings' && hasUpdate}
          <span class="nav-update-dot" title="Доступно обновление"></span>
        {/if}
      </button>
    {/each}
  </nav>
  <div class="sidebar-footer">
    {#if hasUpdate}
      <button
        type="button"
        class="sidebar-update-btn"
        class:is-updating={isUpdating}
        disabled={isUpdating}
        onclick={onDirectUpdate}
        title={isUpdating ? 'Идёт скачивание и установка обновления...' : 'Скачать и установить обновление сразу'}
      >
        {#if isUpdating}
          <div class="sidebar-update-spinner"></div>
          <span>Обновление...</span>
        {:else}
          <svg class="update-btn-icon" viewBox="0 0 16 16" width="13" height="13" fill="currentColor" aria-hidden="true">
            <path d="M8 1a7 7 0 1 0 7 7A7 7 0 0 0 8 1zm0 12.5A5.5 5.5 0 1 1 13.5 8 5.5 5.5 0 0 1 8 13.5zm.75-8.25a.75.75 0 0 0-1.5 0v3.69L5.53 7.22a.75.75 0 0 0-1.06 1.06l3 3a.75.75 0 0 0 1.06 0l3-3a.75.75 0 1 0-1.06-1.06L8.75 8.94z"/>
          </svg>
          <span>Обновить</span>
          {#if updateVersion}
            <span class="update-ver-chip">{updateVersion}</span>
          {/if}
        {/if}
      </button>
    {/if}
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
      {appVersion}
      {#if hasUpdate}
        <span class="update-badge-mini" title="Доступно обновление">NEW</span>
      {/if}
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

  .sidebar-update-btn {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    gap: 6px;
    padding: 7px 12px;
    width: 100%;
    background: linear-gradient(135deg, rgba(37, 99, 235, 0.3) 0%, rgba(99, 102, 241, 0.35) 100%);
    border: 1px solid rgba(129, 140, 248, 0.5);
    border-radius: 6px;
    color: #fff;
    font-size: 12px;
    font-weight: 600;
    cursor: pointer;
    box-shadow: 0 2px 8px rgba(99, 102, 241, 0.25);
    transition: all 0.2s ease;
    position: relative;
    overflow: hidden;
  }

  .sidebar-update-btn:hover:not(:disabled) {
    background: linear-gradient(135deg, rgba(37, 99, 235, 0.45) 0%, rgba(99, 102, 241, 0.5) 100%);
    border-color: #818cf8;
    box-shadow: 0 4px 14px rgba(99, 102, 241, 0.4);
    transform: translateY(-1px);
  }

  .sidebar-update-btn.is-updating {
    cursor: wait;
    opacity: 0.9;
    background: linear-gradient(135deg, rgba(37, 99, 235, 0.5) 0%, rgba(99, 102, 241, 0.55) 100%);
    border-color: #818cf8;
  }

  .sidebar-update-spinner {
    width: 12px;
    height: 12px;
    border: 2px solid rgba(255, 255, 255, 0.3);
    border-top-color: #ffffff;
    border-radius: 50%;
    animation: sidebar-spin 0.8s linear infinite;
    flex-shrink: 0;
  }

  @keyframes sidebar-spin {
    to {
      transform: rotate(360deg);
    }
  }

  .sidebar-update-btn:active {
    transform: translateY(0);
    box-shadow: 0 1px 4px rgba(99, 102, 241, 0.3);
  }

  .update-btn-icon {
    flex-shrink: 0;
    color: #60a5fa;
    animation: bounce-subtle 2s infinite ease-in-out;
  }

  .update-ver-chip {
    font-size: 10px;
    font-weight: 700;
    padding: 1px 5px;
    border-radius: 4px;
    background: rgba(255, 255, 255, 0.18);
    color: #fff;
    letter-spacing: 0.02em;
    margin-left: 2px;
  }

  @keyframes bounce-subtle {
    0%, 100% {
      transform: translateY(0);
    }
    50% {
      transform: translateY(2px);
    }
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
