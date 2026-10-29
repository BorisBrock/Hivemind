<script lang="ts">
  import { open } from "@tauri-apps/plugin-dialog";
  import { app, errorMessage } from "$lib/app.svelte";
  import { commands, PALETTE_SIZE } from "$lib/bindings";
  import { PALETTES, THEMES } from "$lib/theme";
  import Modal from "./Modal.svelte";

  const AGENTS = [
    { id: "claude", label: "Claude Code", available: true },
    { id: "opencode", label: "OpenCode", available: false },
  ];

  // Text fields are saved when they lose focus; everything else applies immediately.
  let claudePath = $state(app.settings.claudePath);
  let model = $state(app.settings.model);
  let check = $state<{ ok: boolean; text: string } | null>(null);
  let checking = $state(false);

  const close = () => (app.settingsOpen = false);

  async function saveText() {
    if (claudePath !== app.settings.claudePath || model !== app.settings.model) {
      check = null;
      await app.updateSettings({ claudePath: claudePath.trim(), model: model.trim() });
    }
  }

  async function browse() {
    const picked = await open({
      title: "Choose the claude executable",
      defaultPath: claudePath || undefined,
    });
    if (typeof picked === "string") {
      claudePath = picked;
      await saveText();
    }
  }

  async function runCheck() {
    checking = true;
    try {
      check = { ok: true, text: await commands.checkAgent(claudePath) };
    } catch (e) {
      check = { ok: false, text: errorMessage(e) };
    }
    checking = false;
  }
</script>

<Modal title="Settings" onclose={close}>
  <div class="space-y-6">
    <section>
      <h3 class="mb-2 text-xs font-semibold tracking-wider text-muted uppercase">Theme</h3>
      <div class="grid grid-cols-3 gap-2">
        {#each THEMES as theme (theme.id)}
          <!-- Each card sets data-theme on itself, so it previews that theme's colours. -->
          <button
            data-theme={theme.id}
            class="rounded-lg border-2 bg-(--wz-bg) p-2 text-left text-(--wz-fg) transition"
            class:border-accent={app.settings.theme === theme.id}
            class:border-transparent={app.settings.theme !== theme.id}
            aria-pressed={app.settings.theme === theme.id}
            onclick={() => app.updateSettings({ theme: theme.id })}
          >
            <div class="mb-2 flex h-10 gap-1">
              <span class="w-1/4 rounded bg-(--wz-surface)"></span>
              <span class="flex-1 space-y-1 rounded bg-(--wz-surface) p-1">
                <span class="block h-2 rounded bg-(--wz-raised)"></span>
                <span class="block h-2 w-2/3 rounded bg-(--wz-accent)"></span>
              </span>
            </div>
            <span class="text-sm font-medium">{theme.label}</span>
          </button>
        {/each}
      </div>
    </section>

    <section>
      <h3 class="mb-2 text-xs font-semibold tracking-wider text-muted uppercase">Ticket colours</h3>
      <div class="space-y-2">
        {#each PALETTES as palette (palette.id)}
          <button
            data-palette={palette.id}
            class="flex w-full items-center gap-3 rounded-lg border-2 bg-raised px-3 py-2"
            class:border-accent={app.settings.palette === palette.id}
            class:border-transparent={app.settings.palette !== palette.id}
            aria-pressed={app.settings.palette === palette.id}
            onclick={() => app.updateSettings({ palette: palette.id })}
          >
            <span class="w-16 text-left text-sm font-medium">{palette.label}</span>
            <span class="flex gap-1">
              {#each Array(PALETTE_SIZE) as _, i (i)}
                <span class="size-4 rounded-full" style:background="var(--p-{i})"></span>
              {/each}
            </span>
          </button>
        {/each}
      </div>
    </section>

    <section class="space-y-3">
      <h3 class="text-xs font-semibold tracking-wider text-muted uppercase">Coding agent</h3>
      <div class="flex gap-2">
        {#each AGENTS as agent (agent.id)}
          <label
            class="flex flex-1 items-center gap-2 rounded-lg border-2 bg-raised px-3 py-2 text-sm"
            class:border-accent={app.settings.agent === agent.id}
            class:border-transparent={app.settings.agent !== agent.id}
            class:opacity-50={!agent.available}
          >
            <input
              type="radio"
              name="agent"
              value={agent.id}
              checked={app.settings.agent === agent.id}
              disabled={!agent.available}
              onchange={() => app.updateSettings({ agent: agent.id })}
            />
            {agent.label}
            {#if !agent.available}<span class="ml-auto text-xs text-muted">coming soon</span>{/if}
          </label>
        {/each}
      </div>

      <label class="block">
        <span class="mb-1 block text-xs font-medium text-muted">Claude Code executable</span>
        <div class="flex gap-2">
          <input
            class="min-w-0 flex-1 rounded-md border border-line bg-raised px-3 py-2 font-mono text-sm outline-none focus:border-accent"
            bind:value={claudePath}
            onblur={saveText}
            placeholder="Found on PATH"
          />
          <button type="button" class="rounded-md border border-line px-3 hover:bg-raised" onclick={browse}>
            Browse…
          </button>
          <button
            type="button"
            class="rounded-md border border-line px-3 hover:bg-raised disabled:opacity-50"
            disabled={checking}
            onclick={runCheck}
          >
            {checking ? "Checking…" : "Check"}
          </button>
        </div>
        {#if check}
          <p class="mt-1 text-xs break-all" class:text-danger={!check.ok} class:text-muted={check.ok}>
            {check.ok ? "✓ " : ""}{check.text}
          </p>
        {/if}
      </label>

      <label class="block">
        <span class="mb-1 block text-xs font-medium text-muted">Model</span>
        <input
          class="w-full rounded-md border border-line bg-raised px-3 py-2 font-mono text-sm outline-none focus:border-accent"
          bind:value={model}
          onblur={saveText}
          placeholder="Claude Code's default (e.g. sonnet, opus, haiku)"
        />
      </label>

      <p class="rounded-lg border border-danger/40 bg-danger/10 p-3 text-xs leading-relaxed">
        <strong>Agents run fully autonomously.</strong> They can read, change and delete files and run any command
        your user account may run, also outside the project folder. Use version control so you can review and undo
        their changes.
      </p>
    </section>
  </div>

  {#snippet footer()}
    <button class="ml-auto rounded-md bg-accent px-4 py-1.5 font-medium text-bg" onclick={close}>Done</button>
  {/snippet}
</Modal>
