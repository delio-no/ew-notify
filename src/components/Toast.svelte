<script lang="ts">
  let { message = '' }: { message?: string } = $props();
</script>

{#if message}
  <div class="toast" role="status">{message}</div>
{/if}
