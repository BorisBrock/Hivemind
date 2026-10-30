<script lang="ts">
  import { untrack } from "svelte";
  import { app, type GroupDialogState } from "$lib/app.svelte";
  import { confirm } from "$lib/confirm.svelte";
  import Modal from "./Modal.svelte";

  let { dialog }: { dialog: GroupDialogState } = $props();

  app.error = null;
  // The dialog edits a snapshot taken when it opens.
  const existing = untrack(() => (dialog.mode === "edit" ? dialog.group : null));

  let name = $state(existing?.name ?? "");
  let saving = $state(false);

  const close = () => (app.groupDialog = null);

  async function save(e: SubmitEvent) {
    e.preventDefault();
    saving = true;
    const ok = existing ? await app.updateGroup(existing.id, { name }) : await app.createGroup(name);
    saving = false;
    if (ok) close();
  }

  async function remove() {
    if (!existing) return;
    const projectCount = app.projects.filter((p) => p.groupId === existing.id).length;
    const confirmed = await confirm({
      title: "Delete group",
      message: `Delete the group "${existing.name}"? Its ${projectCount} project(s) are kept and moved out of the group.`,
      okLabel: "Delete",
      danger: true,
    });
    if (confirmed && (await app.deleteGroup(existing.id))) close();
  }
</script>

<Modal title={existing ? "Edit group" : "New group"} onclose={close}>
  <form id="group-form" class="space-y-4" onsubmit={save}>
    <label class="block">
      <span class="mb-1 block text-xs font-medium text-muted">Name</span>
      <input
        class="w-full rounded-md border border-line bg-raised px-3 py-2 outline-none focus:border-accent"
        bind:value={name}
        placeholder="My group"
        required
        data-autofocus
      />
    </label>

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
      form="group-form"
      class="rounded-md bg-accent px-4 py-1.5 font-medium text-bg disabled:opacity-50"
      disabled={saving}
    >
      {existing ? "Save" : "Create"}
    </button>
  {/snippet}
</Modal>
