<script lang="ts">
  import { open } from "@tauri-apps/plugin-dialog";
  import { untrack } from "svelte";
  import { app, type ProjectDialogState } from "$lib/app.svelte";
  import { PALETTE_SIZE } from "$lib/bindings";
  import { confirm } from "$lib/confirm.svelte";
  import { projectColor } from "$lib/theme";
  import Modal from "./Modal.svelte";

  let { dialog }: { dialog: ProjectDialogState } = $props();

  app.error = null;
  // The dialog edits a snapshot taken when it opens.
  const existing = untrack(() => (dialog.mode === "edit" ? dialog.project : null));
  const usedColors = new Set(app.projects.filter((p) => p.id !== existing?.id).map((p) => p.colorIndex));
  const firstFree = [...Array(PALETTE_SIZE).keys()].find((c) => !usedColors.has(c)) ?? 0;

  let name = $state(existing?.name ?? "");
  let path = $state(existing?.path ?? "");
  let colorIndex = $state(existing?.colorIndex ?? firstFree);
  let saving = $state(false);

  const close = () => (app.projectDialog = null);

  async function browse() {
    const picked = await open({
      directory: true,
      title: "Choose project folder",
      defaultPath: path || undefined,
    });
    if (typeof picked !== "string") return;
    path = picked;
    // Default the name to the folder name.
    if (!name.trim()) name = picked.split(/[\\/]/).filter(Boolean).pop() ?? "";
  }

  async function save(e: SubmitEvent) {
    e.preventDefault();
    saving = true;
    const ok = existing
      ? await app.updateProject(existing.id, { name, path, colorIndex })
      : await app.createProject({ name, path, colorIndex });
    saving = false;
    if (ok) close();
  }

  async function remove() {
    if (!existing) return;
    const ticketCount = app.tickets.filter((t) => t.projectId === existing.id).length;
    const confirmed = await confirm({
      title: "Delete project",
      message: `Delete "${existing.name}" and its ${ticketCount} ticket(s)? Files in the folder are not touched.`,
      okLabel: "Delete",
      danger: true,
    });
    if (confirmed && (await app.deleteProject(existing.id))) close();
  }
</script>

<Modal title={existing ? "Edit project" : "New project"} onclose={close}>
  <form id="project-form" class="space-y-4" onsubmit={save}>
    <label class="block">
      <span class="mb-1 block text-xs font-medium text-muted">Folder</span>
      <div class="flex gap-2">
        <input
          class="min-w-0 flex-1 rounded-md border border-line bg-raised px-3 py-2 font-mono text-sm outline-none focus:border-accent"
          bind:value={path}
          placeholder="/path/to/project"
          required
        />
        <button type="button" class="rounded-md border border-line px-3 hover:bg-raised" onclick={browse}>
          Browse…
        </button>
      </div>
    </label>

    <label class="block">
      <span class="mb-1 block text-xs font-medium text-muted">Name</span>
      <input
        class="w-full rounded-md border border-line bg-raised px-3 py-2 outline-none focus:border-accent"
        bind:value={name}
        placeholder="My project"
        required
      />
    </label>

    <fieldset>
      <legend class="mb-2 text-xs font-medium text-muted">Colour</legend>
      <div class="flex flex-wrap gap-2">
        {#each Array(PALETTE_SIZE) as _, i (i)}
          <button
            type="button"
            class="relative size-8 rounded-full ring-offset-2 ring-offset-surface transition"
            class:ring-2={colorIndex === i}
            class:ring-fg={colorIndex === i}
            style:background={projectColor(i)}
            title={usedColors.has(i) ? "Used by another project" : "Free"}
            aria-label={`Colour ${i + 1}`}
            aria-pressed={colorIndex === i}
            onclick={() => (colorIndex = i)}
          >
            {#if usedColors.has(i)}
              <span class="absolute inset-0 m-auto size-2 rounded-full bg-black/40"></span>
            {/if}
          </button>
        {/each}
      </div>
    </fieldset>

    {#if app.error}
      <p class="text-sm text-danger">{app.error}</p>
    {/if}
  </form>

  {#snippet footer()}
    {#if existing}
      <button class="rounded-md px-3 py-1.5 text-danger hover:bg-raised" onclick={remove}>Delete</button>
    {/if}
    <button class="ml-auto rounded-md px-3 py-1.5 hover:bg-raised" onclick={close}>Cancel</button>
    <button
      type="submit"
      form="project-form"
      class="rounded-md bg-accent px-4 py-1.5 font-medium text-bg disabled:opacity-50"
      disabled={saving}
    >
      {existing ? "Save" : "Create"}
    </button>
  {/snippet}
</Modal>
