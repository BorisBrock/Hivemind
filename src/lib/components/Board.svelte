<script lang="ts">
  import { COLUMNS } from "$lib/app.svelte";
  import Column from "./Column.svelte";
</script>

<main class="flex min-w-0 flex-1 gap-3 overflow-x-auto p-3">
  {#each COLUMNS as column (column.status)}
    <Column {...column} />
  {/each}
</main>
