<script lang="ts">
  import { app, type Ticket } from "$lib/app.svelte";
  import { projectColor } from "$lib/theme";

  let { ticket }: { ticket: Ticket } = $props();
  const project = $derived(app.projectsById.get(ticket.projectId));
  // First non-heading line of the prompt as a preview.
  const preview = $derived(
    ticket.body
      .split("\n")
      .map((l) => l.trim())
      .find((l) => l && !l.startsWith("#")) ?? "",
  );
</script>

<article
  class="cursor-pointer rounded-lg border border-line border-l-4 border-l-(--project) bg-[color-mix(in_srgb,var(--project)_8%,var(--wz-raised))] p-3 shadow-sm transition-[translate,box-shadow] hover:-translate-y-px hover:shadow-md"
  style:--project={projectColor(project?.colorIndex ?? 9)}
>
  <div class="mb-1 text-[11px] font-medium text-(--project)">{project?.name}</div>
  <h3 class="leading-snug font-medium">{ticket.title}</h3>
  {#if preview}
    <p class="mt-1 line-clamp-2 text-xs text-muted">{preview}</p>
  {/if}
</article>
