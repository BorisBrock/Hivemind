<script lang="ts">
  import { app, ticketMatches, ticketPreview, type Ticket } from "$lib/app.svelte";
  import { confirm } from "$lib/confirm.svelte";
  import { projectColor } from "$lib/theme";
  import Icon from "./Icon.svelte";
  import Modal from "./Modal.svelte";

  let search = $state("");
  /** Project to show; empty string for all. */
  let projectId = $state<number | "">("");

  // Only projects that have something in the trash, in sidebar order.
  const projects = $derived(app.projects.filter((p) => app.trash.some((t) => t.projectId === p.id)));
  const shown = $derived(
    app.trash.filter(
      (t) =>
        (projectId === "" || t.projectId === projectId) &&
        ticketMatches(t, app.projectsById.get(t.projectId)?.name ?? "", search),
    ),
  );

  const removedAt = (t: Ticket) =>
    t.archivedAt
      ? new Date(t.archivedAt).toLocaleString(undefined, { dateStyle: "medium", timeStyle: "short" })
      : "";

  async function remove(t: Ticket) {
    const confirmed = await confirm({
      title: "Delete ticket",
      message: `Delete "${t.title}" for good? The ticket and its history cannot be restored.`,
      okLabel: "Delete",
      danger: true,
    });
    if (confirmed) await app.deleteTicket(t.id);
  }

  async function empty() {
    const n = app.trash.length;
    const confirmed = await confirm({
      title: "Empty trash",
      message: `Delete all ${n} ${n === 1 ? "ticket" : "tickets"} in the trash for good? They and their history cannot be restored.`,
      okLabel: "Empty trash",
      danger: true,
    });
    if (confirmed) await app.emptyTrash();
  }

  const open = (t: Ticket) => (app.ticketDialog = { mode: "view", ticketId: t.id });
</script>

<Modal title="Trash" onclose={() => (app.trashOpen = false)} wide>
  <div class="mb-4 flex items-center gap-3">
    <input
      type="search"
      class="w-72 rounded-md border border-line bg-surface px-3 py-1.5 text-sm outline-none focus:border-accent"
      placeholder="Search the trash…"
      data-autofocus
      bind:value={search}
      onkeydown={(e) => {
        // Escape first clears the search, then closes the dialog.
        if (e.key === "Escape" && search) {
          e.preventDefault();
          e.stopPropagation();
          search = "";
        }
      }}
    />
    {#if projects.length > 1}
      <select
        class="rounded-md border border-line bg-surface px-2 py-1.5 text-sm outline-none focus:border-accent"
        bind:value={projectId}
        aria-label="Project"
      >
        <option value="">All projects</option>
        {#each projects as p (p.id)}
          <option value={p.id}>{p.name}</option>
        {/each}
      </select>
    {/if}
    <span class="ml-auto text-xs text-muted">
      {#if shown.length === app.trash.length}
        {app.trash.length} {app.trash.length === 1 ? "ticket" : "tickets"}
      {:else}
        {shown.length} of {app.trash.length} shown
      {/if}
    </span>
  </div>

  {#if !app.trash.length}
    <p class="py-12 text-center text-sm text-muted">
      The trash is empty. Deleted and closed tickets end up here.
    </p>
  {:else if !shown.length}
    <p class="py-12 text-center text-sm text-muted">No ticket in the trash matches.</p>
  {:else}
    <ul class="space-y-2">
      {#each shown as t (t.id)}
        {@const project = app.projectsById.get(t.projectId)}
        {@const preview = ticketPreview(t.body)}
        <li
          class="flex items-start gap-3 rounded-lg border border-line border-l-4 border-l-(--project) bg-[color-mix(in_srgb,var(--project)_8%,var(--wz-raised))] p-3 shadow-sm"
          style:--project={projectColor(project?.colorIndex ?? 9)}
        >
          <button
            class="min-w-0 flex-1 cursor-pointer text-left"
            title="Open the ticket and its history"
            onclick={() => open(t)}
          >
            <div class="mb-1 flex items-center gap-1.5 text-[11px] font-medium text-project">
              {project?.name}
              {#if t.parallel}
                <span class="rounded-full border border-(--project)/40 px-1.5 text-[10px] text-muted"
                  >parallel</span
                >
              {/if}
              <span class="ml-auto font-normal text-muted">Removed {removedAt(t)}</span>
            </div>
            <h3 class="leading-snug font-medium">{t.title}</h3>
            {#if preview}
              <p class="mt-1 line-clamp-2 text-xs text-muted">{preview}</p>
            {/if}
          </button>
          <div class="flex shrink-0 flex-col gap-1">
            <button
              class="flex items-center gap-1.5 rounded-md border border-line px-2.5 py-1 text-sm hover:bg-surface"
              title="Back to the Planning column"
              onclick={() => app.restoreTicket(t.id)}
            >
              <Icon name="rotate-ccw" size={14} /> Restore
            </button>
            <button
              class="flex items-center gap-1.5 rounded-md px-2.5 py-1 text-sm text-danger hover:bg-surface"
              title="Delete the ticket and its history for good"
              onclick={() => remove(t)}
            >
              <Icon name="trash" size={14} /> Delete
            </button>
          </div>
        </li>
      {/each}
    </ul>
  {/if}

  {#snippet footer()}
    <button
      class="ml-auto flex items-center gap-1.5 rounded-md px-3 py-1.5 text-danger hover:bg-raised disabled:opacity-40"
      disabled={!app.trash.length}
      onclick={empty}
    >
      <Icon name="trash" size={14} /> Empty trash
    </button>
  {/snippet}
</Modal>
