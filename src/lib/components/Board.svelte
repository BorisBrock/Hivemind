<script lang="ts">
  import { app, COLUMNS } from "$lib/app.svelte";
  import Column from "./Column.svelte";

  const shown = $derived([...app.columns.values()].reduce((n, list) => n + list.length, 0));
</script>

<main class="flex min-w-0 flex-1 flex-col">
  <div class="flex items-center gap-3 px-3 pt-3">
    <input
      id="board-search"
      type="search"
      class="w-72 rounded-md border border-line bg-surface px-3 py-1.5 text-sm outline-none focus:border-accent"
      placeholder="Search tickets…  ( / )"
      bind:value={app.search}
      onkeydown={(e) => {
        if (e.key === "Escape") {
          app.search = "";
          e.currentTarget.blur();
        }
      }}
    />
    {#if app.search.trim()}
      <span class="text-xs text-muted">{shown} matching</span>
    {/if}
    <span class="ml-auto text-xs text-muted">
      <kbd>N</kbd> new ticket · <kbd>P</kbd> new project · <kbd>Ctrl</kbd>+<kbd>,</kbd> settings
    </span>
  </div>
  <div class="flex min-h-0 flex-1 gap-3 overflow-x-auto p-3">
    {#each COLUMNS as column (column.status)}
      <Column {...column} />
    {/each}
  </div>
</main>
