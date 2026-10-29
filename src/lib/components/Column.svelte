<script lang="ts">
  import { dndzone, TRIGGERS, type DndEvent } from "svelte-dnd-action";
  import { flip } from "svelte/animate";
  import { app, userCanMove, type Ticket } from "$lib/app.svelte";
  import type { TicketStatus } from "$lib/bindings";
  import Icon from "./Icon.svelte";
  import TicketCard from "./TicketCard.svelte";

  let { status, label }: { status: TicketStatus; label: string } = $props();

  const FLIP_MS = 150;
  // Only the agent (or the ticket view, for Done) moves tickets out of these columns.
  const locked = $derived(status === "in_progress" || status === "needs_input" || status === "done");

  const source = $derived(app.columns.get(status) ?? []);
  // Local copy that svelte-dnd-action rearranges while a drag is in progress.
  let items = $state<Ticket[]>([]);
  $effect.pre(() => {
    if (!app.dragging) items = source;
  });

  const accepts = $derived(!app.dragging || userCanMove(app.dragging.from, status));

  function consider(e: CustomEvent<DndEvent<Ticket>>) {
    items = e.detail.items;
    const { trigger, id } = e.detail.info;
    if (trigger === TRIGGERS.DRAG_STARTED) app.dragging = { id: Number(id), from: status };
  }

  async function finalize(e: CustomEvent<DndEvent<Ticket>>) {
    items = e.detail.items;
    const id = Number(e.detail.info.id);
    const index = items.findIndex((t) => t.id === id);
    // Both the source and the target column finalize; only the one now holding the ticket
    // moves it, and it ends the drag afterwards so the board doesn't flash the old order.
    if (index < 0) return;
    await app.moveTicket(id, status, items[index + 1]?.id ?? null);
    app.dragging = null;
  }
</script>

<section
  class="flex w-72 min-w-60 flex-1 flex-col rounded-xl bg-surface/60 transition-opacity"
  class:opacity-40={!accepts}
>
  <header class="flex h-10 items-center gap-2 px-3">
    <h2 class="text-sm font-semibold">{label}</h2>
    <span class="rounded-full bg-raised px-2 text-xs text-muted">{source.length}</span>
    {#if status === "planning"}
      <button
        class="ml-auto rounded-md p-1 text-muted hover:bg-raised hover:text-fg disabled:opacity-40"
        title={app.projects.some((p) => !p.hidden) ? "New ticket" : "Create a project first"}
        disabled={!app.projects.some((p) => !p.hidden)}
        onclick={() => (app.ticketDialog = { mode: "create" })}
      >
        <Icon name="plus" />
      </button>
    {/if}
  </header>
  <div
    class="flex-1 space-y-2 overflow-y-auto px-2 pb-2"
    use:dndzone={{
      items,
      type: "ticket",
      flipDurationMs: FLIP_MS,
      dragDisabled: locked,
      dropFromOthersDisabled: !accepts,
      dropTargetStyle: { outline: "2px dashed var(--wz-line)", outlineOffset: "-2px", borderRadius: "12px" },
    }}
    onconsider={consider}
    onfinalize={finalize}
  >
    {#each items as ticket (ticket.id)}
      <div animate:flip={{ duration: FLIP_MS }}>
        <TicketCard {ticket} />
      </div>
    {/each}
  </div>
</section>
