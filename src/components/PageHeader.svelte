<script lang="ts">
  import type { Snippet } from 'svelte';

  let {
    title,
    children,
  }: {
    title: string;
    children?: Snippet;
  } = $props();
</script>

<div class="page-header">
  <h1>{title}</h1>
  {#if children}
    <div class="actions">{@render children()}</div>
  {/if}
</div>
