<script lang="ts">
  import type { Snippet } from 'svelte';

  let {
    title = '',
    className = '',
    header,
    children,
  }: {
    title?: string;
    className?: string;
    header?: Snippet;
    children?: Snippet;
  } = $props();
</script>

<div class="card {className}">
  {#if title || header}
    <div class="card-header">
      {#if title}<h2>{title}</h2>{/if}
      {#if header}{@render header()}{/if}
    </div>
  {/if}
  <div class="card-body">
    {#if children}{@render children()}{/if}
  </div>
</div>
