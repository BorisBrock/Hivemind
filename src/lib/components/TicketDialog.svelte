<script lang="ts">
  import { ask } from "@tauri-apps/plugin-dialog";
  import { untrack } from "svelte";
  import { app, COLUMNS, type TicketDialogState } from "$lib/app.svelte";
  import { projectColor } from "$lib/theme";
  import Markdown from "./Markdown.svelte";
  import MarkdownEditor from "./MarkdownEditor.svelte";
  import Modal from "./Modal.svelte";

  let { dialog }: { dialog: TicketDialogState } = $props();

  app.error = null;
  const ticket = $derived(
    dialog.mode === "view" ? app.tickets.find((t) => t.id === dialog.ticketId) : undefined,
  );
  const visibleProjects = app.projects.filter((p) => !p.hidden);

  // Draft values, seeded once from the ticket (or defaults for a new one).
  const initial = untrack(() => ({
    ticket: dialog.mode === "view" ? app.tickets.find((t) => t.id === dialog.ticketId) : undefined,
    projectId: dialog.mode === "create" ? dialog.projectId : undefined,
  }));
  let projectId = $state(initial.ticket?.projectId ?? initial.projectId ?? visibleProjects[0]?.id);
  let title = $state(initial.ticket?.title ?? "");
  let body = $state(initial.ticket?.body ?? "");

  const editable = $derived(dialog.mode === "create" || ticket?.status === "planning");
  const project = $derived(projectId !== undefined ? app.projectsById.get(projectId) : undefined);
  const statusLabel = $derived(COLUMNS.find((c) => c.status === ticket?.status)?.label);
  const dirty = $derived(!!ticket && (title !== ticket.title || body !== ticket.body));

  const close = () => (app.ticketDialog = null);

  /** Creates or saves the ticket; optionally sends it to Ready afterwards. */
  async function save(sendToReady: boolean) {
    let saved;
    if (dialog.mode === "create") {
      if (projectId === undefined) return;
      saved = await app.createTicket({ projectId, title, body });
    } else if (ticket) {
      saved = dirty ? await app.updateTicket(ticket.id, { title, body }) : ticket;
    }
    if (!saved) return;
    if (sendToReady && !(await app.moveTicket(saved.id, "ready"))) return;
    close();
  }

  async function move(to: "planning" | "ready") {
    if (ticket && (await app.moveTicket(ticket.id, to))) close();
  }

  async function remove(verb: "Delete" | "Close") {
    if (!ticket) return;
    const confirmed = await ask(`${verb} "${ticket.title}"? The ticket and its history are deleted.`, {
      title: `${verb} ticket`,
      kind: "warning",
      okLabel: verb,
      cancelLabel: "Cancel",
    });
    if (confirmed && (await app.deleteTicket(ticket.id))) close();
  }
</script>

<Modal title={dialog.mode === "create" ? "New ticket" : (ticket?.title ?? "Ticket")} onclose={close} wide>
  <div class="flex h-full flex-col gap-4" style:--project={projectColor(project?.colorIndex ?? 9)}>
    <div class="flex items-center gap-3">
      {#if dialog.mode === "create"}
        <select
          class="rounded-md border border-line bg-raised px-2 py-1.5 text-sm"
          bind:value={projectId}
          aria-label="Project"
        >
          {#each visibleProjects as p (p.id)}<option value={p.id}>{p.name}</option>{/each}
        </select>
      {:else}
        <span class="rounded-full bg-(--project)/15 px-2.5 py-0.5 text-xs font-medium text-(--project)">
          {project?.name}
        </span>
        <span class="rounded-full bg-raised px-2.5 py-0.5 text-xs text-muted">{statusLabel}</span>
      {/if}
    </div>

    {#if editable}
      <input
        class="w-full border-b border-line bg-transparent pb-2 text-xl font-semibold outline-none focus:border-(--project)"
        bind:value={title}
        placeholder="Ticket title"
        aria-label="Title"
      />
      <div
        class="min-h-0 flex-1 rounded-lg border border-line bg-raised px-3 focus-within:border-(--project)"
      >
        <MarkdownEditor
          bind:value={body}
          placeholder="Describe the task for the agent. Markdown is supported."
          autofocus={dialog.mode === "view"}
        />
      </div>
    {:else if ticket}
      <section>
        <h3 class="mb-2 text-xs font-semibold tracking-wider text-muted uppercase">Prompt</h3>
        <div class="rounded-lg border border-line bg-raised p-4">
          <Markdown source={ticket.body} />
        </div>
      </section>
      <section class="flex min-h-40 flex-1 flex-col">
        <h3 class="mb-2 text-xs font-semibold tracking-wider text-muted uppercase">Agent conversation</h3>
        <div
          class="flex flex-1 items-center justify-center rounded-lg border border-dashed border-line text-sm text-muted"
        >
          The agent's conversation will appear here.
        </div>
      </section>
    {/if}

    {#if app.error}
      <p class="text-sm text-danger">{app.error}</p>
    {/if}
  </div>

  {#snippet footer()}
    {#if dialog.mode === "create"}
      <button class="ml-auto rounded-md px-3 py-1.5 hover:bg-raised" onclick={close}>Cancel</button>
      <button class="rounded-md border border-line px-3 py-1.5 hover:bg-raised" onclick={() => save(false)}>
        Create
      </button>
      <button class="rounded-md bg-accent px-4 py-1.5 font-medium text-bg" onclick={() => save(true)}>
        Create &amp; send to Ready
      </button>
    {:else if ticket?.status === "planning"}
      <button class="rounded-md px-3 py-1.5 text-danger hover:bg-raised" onclick={() => remove("Delete")}>
        Delete
      </button>
      <button
        class="ml-auto rounded-md border border-line px-3 py-1.5 hover:bg-raised disabled:opacity-50"
        disabled={!dirty}
        onclick={() => save(false)}
      >
        Save
      </button>
      <button class="rounded-md bg-accent px-4 py-1.5 font-medium text-bg" onclick={() => save(true)}>
        Send to Ready
      </button>
    {:else if ticket?.status === "ready"}
      <button class="rounded-md px-3 py-1.5 text-danger hover:bg-raised" onclick={() => remove("Delete")}>
        Delete
      </button>
      <button
        class="ml-auto rounded-md border border-line px-3 py-1.5 hover:bg-raised"
        onclick={() => move("planning")}
      >
        Back to Planning
      </button>
    {:else if ticket?.status === "done"}
      <button
        class="ml-auto rounded-md border border-line px-3 py-1.5 hover:bg-raised"
        onclick={() => move("ready")}
      >
        Send back to Ready
      </button>
      <button class="rounded-md bg-accent px-4 py-1.5 font-medium text-bg" onclick={() => remove("Close")}>
        Close ticket
      </button>
    {:else if ticket}
      <button
        class="ml-auto rounded-md border border-line px-3 py-1.5 opacity-50"
        disabled
        title="Not available yet"
      >
        Cancel agent
      </button>
    {/if}
  {/snippet}
</Modal>
