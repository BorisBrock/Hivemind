<script lang="ts">
  import { app, type Ticket } from "$lib/app.svelte";
  import { projectColor } from "$lib/theme";

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
    {#if ticket.status === "in_progress"}
      <span class="ml-auto size-2 animate-pulse rounded-full bg-(--project)" title="Agent working"></span>
    {:else if ticket.status === "needs_input"}
      <span class="ml-auto rounded-full bg-accent px-1.5 text-[10px] text-bg">Your turn</span>
    {/if}
  </div>
  <h3 class="leading-snug font-medium">{ticket.title}</h3>
  {#if preview}
    <p class="mt-1 line-clamp-2 text-xs text-muted">{preview}</p>
  {/if}
</div>
