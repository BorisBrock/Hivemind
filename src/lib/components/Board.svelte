<script lang="ts">
  import { app, COLUMNS } from "$lib/app.svelte";
  import TicketCard from "./TicketCard.svelte";
</script>

<main class="flex min-w-0 flex-1 gap-3 overflow-x-auto p-3">
  {#each COLUMNS as column (column.status)}
    {@const tickets = app.columns.get(column.status) ?? []}
    <section class="flex w-72 min-w-60 flex-1 flex-col rounded-xl bg-surface/60">
      <header class="flex h-10 items-center gap-2 px-3">
        <h2 class="text-sm font-semibold">{column.label}</h2>
        <span class="rounded-full bg-raised px-2 text-xs text-muted">{tickets.length}</span>
      </header>
      <div class="flex-1 space-y-2 overflow-y-auto px-2 pb-2">
        {#each tickets as ticket (ticket.id)}
          <TicketCard {ticket} />
        {/each}
      </div>
    </section>
  {/each}
</main>
