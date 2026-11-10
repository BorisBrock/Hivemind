<script lang="ts">
  import { untrack } from "svelte";
  import { app, COLUMNS, type TicketDialogState } from "$lib/app.svelte";
  import { pastedImages } from "$lib/attachments";
  import { confirm } from "$lib/confirm.svelte";
  import { projectColor } from "$lib/theme";
  import CopyButton from "./CopyButton.svelte";
  import ImageStrip from "./ImageStrip.svelte";
  import Markdown from "./Markdown.svelte";
  import MarkdownEditor from "./MarkdownEditor.svelte";
  import Modal from "./Modal.svelte";
  import Transcript from "./Transcript.svelte";

  let { dialog }: { dialog: TicketDialogState } = $props();

  app.error = null;
  /** Board tickets and, opened from the trash, trashed ones (read-only). */
  const findTicket = (id: number) =>
    app.tickets.find((t) => t.id === id) ?? app.trash.find((t) => t.id === id);
  const ticket = $derived(dialog.mode === "view" ? findTicket(dialog.ticketId) : undefined);
  const trashed = $derived(!!ticket?.archivedAt);
  const visibleProjects = app.projects.filter((p) => !p.hidden);

  // Draft values, seeded once from the ticket (or defaults for a new one).
  const initial = untrack(() => ({
    ticket: dialog.mode === "view" ? findTicket(dialog.ticketId) : undefined,
    projectId: dialog.mode === "create" ? dialog.projectId : undefined,
  }));
  const lastProjectId = visibleProjects.some((p) => p.id === app.lastTicketProjectId)
    ? app.lastTicketProjectId
    : undefined;
  let projectId = $state(
    initial.ticket?.projectId ?? initial.projectId ?? lastProjectId ?? visibleProjects[0]?.id,
  );
  let title = $state(initial.ticket?.title ?? "");
  let body = $state(initial.ticket?.body ?? "");
  let parallel = $state(initial.ticket?.parallel ?? false);

  /** Title used when the title field is left empty: the body's first line, minus Markdown. */
  const fallbackTitle = $derived.by(() => {
    const line = body
      .split("\n")
      .map((l) =>
        l
          .replace(/^\s*(#{1,6}\s+|>\s*|[-*+]\s+(\[[ xX]\]\s+)?|\d+[.)]\s+)*/, "")
          .replace(/!?\[([^\]]*)\]\([^)]*\)/g, "$1")
          .replace(/[*_`~]/g, "")
          .replace(/\s+/g, " ")
          .trim(),
      )
      .find((l) => l);
    if (!line || line.length <= 60) return line ?? "";
    const cut = line.slice(0, 60);
    const space = cut.lastIndexOf(" ");
    return `${space > 30 ? cut.slice(0, space) : cut}…`;
  });
  const effectiveTitle = $derived(title.trim() || fallbackTitle);

  const editable = $derived(dialog.mode === "create" || (ticket?.status === "planning" && !trashed));
  const project = $derived(projectId !== undefined ? app.projectsById.get(projectId) : undefined);
  const statusLabel = $derived(
    trashed ? "In the trash" : COLUMNS.find((c) => c.status === ticket?.status)?.label,
  );
  const dirty = $derived(
    !!ticket && (title !== ticket.title || body !== ticket.body || parallel !== ticket.parallel),
  );

  const transcript = $derived(ticket ? app.transcripts[ticket.id] : undefined);
  $effect(() => {
    const id = ticket?.id;
    if (id !== undefined) untrack(() => app.loadTranscript(id));
  });

  /** Reply to a waiting agent, a message to a working one, or the comment when sending a
   *  Done ticket back. */
  let message = $state("");
  let sending = $state(false);
  const canMessage = $derived(
    !trashed &&
      (ticket?.status === "needs_input" || ticket?.status === "done" || ticket?.status === "in_progress"),
  );

  const close = () => (app.ticketDialog = null);

  /** Anything typed that would be lost by closing without saving. */
  const unsaved = $derived(
    (dialog.mode === "create" ? !!(title.trim() || body.trim()) : editable && dirty) || !!message.trim(),
  );

  /** Asks before discarding unsaved input; true if it's fine to close. */
  async function confirmDiscard() {
    if (!unsaved) return true;
    return confirm({
      title: "Unsaved changes",
      message:
        dialog.mode === "create"
          ? "Discard this new ticket? What you've written so far is lost."
          : "Discard your unsaved changes?",
      okLabel: "Discard",
      cancelLabel: "Keep editing",
      danger: true,
    });
  }

  async function cancelCreate() {
    if (await confirmDiscard()) close();
  }

  async function send(text = message) {
    if (!ticket || !text.trim()) return;
    sending = true;
    const sendingBack = ticket.status === "done";
    const ok = sendingBack
      ? await app.sendBackToReady(ticket.id, text)
      : await app.replyToTicket(ticket.id, text);
    sending = false;
    if (!ok) return;
    message = "";
    // A Done ticket sent back to Ready has left the user's hands; get out of the way.
    if (sendingBack) close();
  }

  /** Pasting an image into the reply inserts a link to it at the cursor. */
  function pasteIntoMessage(e: ClipboardEvent) {
    const input = e.currentTarget as HTMLTextAreaElement;
    const { selectionStart, selectionEnd } = input;
    pastedImages(e)?.then((text) => {
      if (!text) return;
      message = message.slice(0, selectionStart) + text + message.slice(selectionEnd);
      const cursor = selectionStart + text.length;
      requestAnimationFrame(() => input.setSelectionRange(cursor, cursor));
    });
  }

  async function cancel() {
    if (!ticket) return;
    const confirmed = await confirm({
      title: "Cancel agent",
      message: `Stop the agent working on "${ticket.title}"? The ticket goes back to Planning; changes the agent already made to files stay.`,
      okLabel: "Stop agent",
      cancelLabel: "Keep running",
      kind: "question",
      danger: true,
    });
    if (confirmed) await app.cancelTicket(ticket.id);
  }

  /** Creates or saves the ticket; optionally sends it to Ready afterwards. */
  async function save(sendToReady: boolean) {
    let saved;
    if (dialog.mode === "create") {
      if (projectId === undefined) return;
      saved = await app.createTicket({ projectId, title: effectiveTitle, body, parallel });
      if (saved) app.lastTicketProjectId = projectId;
    } else if (ticket) {
      saved = dirty ? await app.updateTicket(ticket.id, { title: effectiveTitle, body, parallel }) : ticket;
    }
    if (!saved) return;
    if (sendToReady && !(await app.moveTicket(saved.id, "ready"))) return;
    close();
  }

  /** Ctrl+S saves, Ctrl+Enter saves and sends to Ready (while the ticket is editable). */
  function editorShortcut(e: KeyboardEvent) {
    if (!editable || !(e.ctrlKey || e.metaKey)) return;
    if (e.key.toLowerCase() === "s" && (dialog.mode === "create" || dirty)) {
      e.preventDefault();
      e.stopPropagation();
      save(false);
    } else if (e.key === "Enter") {
      e.preventDefault();
      e.stopPropagation();
      save(true);
    }
  }

  async function move(to: "planning") {
    if (ticket && (await app.moveTicket(ticket.id, to))) close();
  }

  /** Deleting or closing moves the ticket to the trash, so only unsaved input is worth asking about. */
  async function trash() {
    if (ticket && (await confirmDiscard()) && (await app.trashTicket(ticket.id))) close();
  }

  async function deleteForGood() {
    if (!ticket) return;
    const confirmed = await confirm({
      title: "Delete ticket",
      message: `Delete "${ticket.title}" for good? The ticket and its history cannot be restored.`,
      okLabel: "Delete",
      danger: true,
    });
    if (confirmed && (await app.deleteTicket(ticket.id))) close();
  }
</script>

{#snippet parallelToggle()}
  <label
    class="flex cursor-pointer items-center gap-2 text-sm text-muted select-none hover:text-fg"
    title="Start as soon as it is Ready, alongside other tickets of this project. Only for tickets that change no files (questions, analysis)."
  >
    <input type="checkbox" class="accent-accent" bind:checked={parallel} />
    Process in parallel
  </label>
{/snippet}

<Modal
  title={dialog.mode === "create" ? "New ticket" : (ticket?.title ?? "Ticket")}
  accent={projectColor(project?.colorIndex ?? 9)}
  onclose={close}
  canclose={confirmDiscard}
  wide
>
  <!-- Capture phase, so the editor doesn't take Ctrl+Enter for itself. -->
  <div
    class="flex h-full flex-col gap-4"
    style:--project={projectColor(project?.colorIndex ?? 9)}
    onkeydowncapture={editorShortcut}
    role="presentation"
  >
    <div class="flex items-center gap-3">
      {#if dialog.mode === "create"}
        <div class="flex min-w-0 flex-1 flex-wrap items-center gap-2" role="radiogroup" aria-label="Project">
          {#each visibleProjects as p (p.id)}
            {@const selected = p.id === projectId}
            <button
              type="button"
              role="radio"
              aria-checked={selected}
              style:--chip={projectColor(p.colorIndex)}
              class={[
                "flex items-center gap-2 rounded-full border px-3 py-1 text-sm transition-colors",
                selected
                  ? "border-(--chip) bg-(--chip)/15 font-medium text-fg"
                  : "border-line text-muted hover:bg-raised hover:text-fg",
              ]}
              onclick={() => (projectId = p.id)}
            >
              <span class="size-2 rounded-full bg-(--chip)"></span>
              {p.name}
            </button>
          {/each}
        </div>
      {:else}
        <span class="rounded-full bg-(--project)/15 px-2.5 py-0.5 text-xs font-medium text-project">
          {project?.name}
        </span>
        <span class="rounded-full bg-raised px-2.5 py-0.5 text-xs text-muted">{statusLabel}</span>
      {/if}
    </div>

    {#if editable}
      <input
        class="w-full border-b border-line bg-transparent pb-2 text-xl font-semibold outline-none focus:border-(--project)"
        bind:value={title}
        placeholder={fallbackTitle || "Ticket title"}
        aria-label="Title"
        data-autofocus={dialog.mode === "create" || undefined}
        spellcheck={false}
      />
      <div
        class="min-h-0 flex-1 rounded-lg border border-line bg-raised px-3 focus-within:border-(--project)"
      >
        <MarkdownEditor
          bind:value={body}
          placeholder="Describe the task for the agent. Markdown is supported; paste images with Ctrl+V."
          autofocus={dialog.mode === "view"}
        />
      </div>
      <ImageStrip text={body} />
    {:else if ticket}
      {#if transcript?.length}
        <div class="min-h-0 flex-1">
          <Transcript
            events={transcript}
            cwd={app.projects.find((p) => p.id === ticket.projectId)?.path}
            working={ticket.status === "in_progress"}
            onoption={ticket.status === "needs_input" ? send : undefined}
          />
        </div>
      {:else}
        <section class="min-h-0 flex-1 overflow-y-auto">
          <h3 class="mb-2 text-xs font-semibold tracking-wider text-muted uppercase">Prompt</h3>
          <div class="message-block rounded-lg border border-line bg-raised p-4">
            <Markdown source={ticket.body} />
            <CopyButton text={ticket.body} />
          </div>
          {#if ticket.status === "ready" && !trashed}
            <p class="mt-3 text-sm text-muted">
              {ticket.parallel
                ? "Waiting for the agent: it starts right away, in parallel."
                : "Waiting for the agent: it starts once the project is free."}
            </p>
          {/if}
        </section>
      {/if}

      {#if canMessage}
        <textarea
          class="h-24 w-full shrink-0 resize-none rounded-lg border border-line bg-raised p-3 outline-none focus:border-(--project)"
          bind:value={message}
          spellcheck={false}
          placeholder={ticket.status === "needs_input"
            ? "Answer the agent… (Ctrl+Enter to send)"
            : ticket.status === "in_progress"
              ? ticket.agent === "opencode"
                ? "Message the agent; it reads it once its current turn ends… (Ctrl+Enter to send)"
                : 'Message the agent while it works, e.g. "What are you doing right now?" (Ctrl+Enter to send)'
              : "Ask for a change; the agent continues in the same session… (Ctrl+Enter to send)"}
          onkeydown={(e) => e.key === "Enter" && (e.ctrlKey || e.metaKey) && send()}
          onpaste={pasteIntoMessage}></textarea>
        <ImageStrip text={message} />
      {/if}
    {/if}

    {#if app.error}
      <p class="text-sm text-danger">{app.error}</p>
    {/if}
  </div>

  {#snippet footer()}
    {#if dialog.mode === "create"}
      {@render parallelToggle()}
      <button class="ml-auto rounded-md px-3 py-1.5 hover:bg-raised" onclick={cancelCreate}>Cancel</button>
      <button
        class="rounded-md border border-line px-3 py-1.5 hover:bg-raised"
        title="Ctrl+S"
        onclick={() => save(false)}
      >
        Create
      </button>
      <button
        class="rounded-md bg-accent px-4 py-1.5 font-medium text-bg"
        title="Ctrl+Enter"
        onclick={() => save(true)}
      >
        Create &amp; send to Ready
      </button>
    {:else if trashed}
      <button class="rounded-md px-3 py-1.5 text-danger hover:bg-raised" onclick={deleteForGood}>
        Delete for good
      </button>
      <button
        class="ml-auto rounded-md bg-accent px-4 py-1.5 font-medium text-bg"
        title="Back to the Planning column"
        onclick={() => ticket && app.restoreTicket(ticket.id)}
      >
        Restore
      </button>
    {:else if ticket?.status === "planning"}
      <button
        class="rounded-md px-3 py-1.5 text-danger hover:bg-raised"
        title="Moves the ticket to the trash"
        onclick={trash}
      >
        Delete
      </button>
      {@render parallelToggle()}
      <button
        class="ml-auto rounded-md border border-line px-3 py-1.5 hover:bg-raised disabled:opacity-50"
        disabled={!dirty}
        title="Ctrl+S"
        onclick={() => save(false)}
      >
        Save
      </button>
      <button
        class="rounded-md bg-accent px-4 py-1.5 font-medium text-bg"
        title="Ctrl+Enter"
        onclick={() => save(true)}
      >
        Send to Ready
      </button>
    {:else if ticket?.status === "ready"}
      <button
        class="rounded-md px-3 py-1.5 text-danger hover:bg-raised"
        title="Moves the ticket to the trash"
        onclick={trash}
      >
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
        class="ml-auto rounded-md border border-line px-3 py-1.5 hover:bg-raised disabled:opacity-50"
        disabled={!message.trim() || sending}
        title="Write a comment for the agent first"
        onclick={() => send()}
      >
        Send back to Ready
      </button>
      <button
        class="rounded-md bg-accent px-4 py-1.5 font-medium text-bg"
        title="Moves the ticket to the trash"
        onclick={trash}
      >
        Close ticket
      </button>
    {:else if ticket?.status === "needs_input"}
      <button class="rounded-md px-3 py-1.5 text-danger hover:bg-raised" onclick={cancel}>Cancel agent</button
      >
      <button
        class="ml-auto rounded-md bg-accent px-4 py-1.5 font-medium text-bg disabled:opacity-50"
        disabled={!message.trim() || sending}
        onclick={() => send()}
      >
        Send reply
      </button>
    {:else if ticket?.status === "in_progress"}
      <button class="rounded-md px-3 py-1.5 text-danger hover:bg-raised" onclick={cancel}>Cancel agent</button
      >
      <button
        class="ml-auto rounded-md bg-accent px-4 py-1.5 font-medium text-bg disabled:opacity-50"
        disabled={!message.trim() || sending}
        onclick={() => send()}
      >
        Send message
      </button>
    {/if}
  {/snippet}
</Modal>
