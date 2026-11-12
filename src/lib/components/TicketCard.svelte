<script lang="ts">
  import { app, ticketPreview, type Ticket } from "$lib/app.svelte";
  import { projectColor } from "$lib/theme";
  import ActivityDot from "./ActivityDot.svelte";
  import Icon from "./Icon.svelte";

  let { ticket }: { ticket: Ticket } = $props();
  const project = $derived(app.projectsById.get(ticket.projectId));
  const preview = $derived(ticketPreview(ticket.body));
  // Finished tickets can be closed straight from the board, like "Close ticket" in the dialog.
  const closable = $derived(ticket.status === "done");

  async function close(e: Event) {
    e.stopPropagation();
    await app.trashTicket(ticket.id);
  }
</script>

<div
  role="button"
  tabindex="0"
  onclick={() => (app.ticketDialog = { mode: "view", ticketId: ticket.id })}
  onkeydown={(e) => e.key === "Enter" && (app.ticketDialog = { mode: "view", ticketId: ticket.id })}
  class="group cursor-pointer rounded-lg border border-line border-l-4 border-l-(--project) bg-[color-mix(in_srgb,var(--project)_8%,var(--wz-raised))] p-3 shadow-sm transition-[translate,box-shadow] hover:-translate-y-px hover:shadow-md"
  style:--project={projectColor(project?.colorIndex ?? 9)}
>
  <div class="mb-1 flex items-center gap-1.5 text-[11px] font-medium text-project">
    {project?.name}
    {#if ticket.parallel}
      <span
        class="rounded-full border border-(--project)/40 px-1.5 text-[10px] text-muted"
        title="Runs in parallel">parallel</span
      >
    {/if}
    {#if ticket.status === "in_progress"}
      <span
        class="ml-auto flex items-center gap-1.5 rounded-full border border-(--project)/40 bg-(--project)/15 py-0.5 pr-2 pl-1.5 text-[10px] text-fg"
        title="Agent working"
      >
        <ActivityDot />
        Working
      </span>
    {:else if ticket.status === "needs_input"}
      <span class="ml-auto rounded-full bg-accent px-1.5 text-[10px] text-bg">Your turn</span>
    {:else if closable}
      <button
        class="-my-1 -mr-1 ml-auto rounded p-1 text-muted opacity-0 group-hover:opacity-100 group-focus-within:opacity-100 hover:bg-surface hover:text-danger focus-visible:opacity-100"
        title="Close ticket (moves it to the trash)"
        aria-label="Close ticket"
        onclick={close}
        onkeydown={(e) => e.stopPropagation()}
      >
        <Icon name="x" size={14} />
      </button>
    {/if}
  </div>
  <h3 class="leading-snug font-medium">{ticket.title}</h3>
  {#if preview}
    <p class="mt-1 line-clamp-2 text-xs text-muted">{preview}</p>
  {/if}
</div>
