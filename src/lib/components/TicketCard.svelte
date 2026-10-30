<script lang="ts">
  import { app, type Ticket } from "$lib/app.svelte";
  import { confirm } from "$lib/confirm.svelte";
  import { projectColor } from "$lib/theme";
  import Icon from "./Icon.svelte";

  let { ticket }: { ticket: Ticket } = $props();
  const project = $derived(app.projectsById.get(ticket.projectId));
  // First non-heading line of the prompt as a preview, without markdown syntax.
  const preview = $derived(
    (
      ticket.body
        .split("\n")
        .map((l) => l.trim())
        .find((l) => l && !l.startsWith("#")) ?? ""
    )
      .replace(/^([-*+]|\d+\.|>)\s+/, "")
      .replace(/[*_`~]/g, ""),
  );
  // No agent is attached in these columns, so the ticket can go without cancelling anything.
  const deletable = $derived(ticket.status === "planning" || ticket.status === "done");

  async function remove(e: Event) {
    e.stopPropagation();
    const confirmed = await confirm({
      title: "Delete ticket",
      message: `Delete "${ticket.title}"? The ticket and its history are deleted.`,
      okLabel: "Delete",
      danger: true,
    });
    if (confirmed) await app.deleteTicket(ticket.id);
  }
</script>

<div
  role="button"
  tabindex="0"
  onclick={() => (app.ticketDialog = { mode: "view", ticketId: ticket.id })}
  onkeydown={(e) => e.key === "Enter" && (app.ticketDialog = { mode: "view", ticketId: ticket.id })}
  class="cursor-pointer rounded-lg border border-line border-l-4 border-l-(--project) bg-[color-mix(in_srgb,var(--project)_8%,var(--wz-raised))] p-3 shadow-sm transition-[translate,box-shadow] hover:-translate-y-px hover:shadow-md"
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
      <span class="ml-auto size-2 animate-pulse rounded-full bg-(--project)" title="Agent working"></span>
    {:else if ticket.status === "needs_input"}
      <span class="ml-auto rounded-full bg-accent px-1.5 text-[10px] text-bg">Your turn</span>
    {:else if deletable}
      <button
        class="-my-1 -mr-1 ml-auto rounded p-1 text-muted hover:bg-surface hover:text-danger"
        title="Delete ticket"
        aria-label="Delete ticket"
        onclick={remove}
        onkeydown={(e) => e.stopPropagation()}
      >
        <Icon name="trash" size={14} />
      </button>
    {/if}
  </div>
  <h3 class="leading-snug font-medium">{ticket.title}</h3>
  {#if preview}
    <p class="mt-1 line-clamp-2 text-xs text-muted">{preview}</p>
  {/if}
</div>
