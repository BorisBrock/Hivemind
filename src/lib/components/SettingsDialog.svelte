<script lang="ts">
  import { open } from "@tauri-apps/plugin-dialog";
  import { app, errorMessage } from "$lib/app.svelte";
  import { commands, PALETTE_SIZE } from "$lib/bindings";
  import { PALETTES, THEMES } from "$lib/theme";
  import Modal from "./Modal.svelte";

  const AGENTS = [
    {
      id: "claude",
      label: "Claude Code",
      exe: "claude",
      pathKey: "claudePath",
      modelKey: "claudeModel",
      modelHint: "Claude Code's default (e.g. sonnet, opus, haiku)",
    },
    {
      id: "opencode",
      label: "OpenCode",
      exe: "opencode",
      pathKey: "opencodePath",
      modelKey: "opencodeModel",
      modelHint: "OpenCode's default (provider/model, e.g. opencode/big-pickle)",
    },
  ] as const;
  type Agent = (typeof AGENTS)[number];

  const agent = $derived(AGENTS.find((a) => a.id === app.settings.agent) ?? AGENTS[0]);

  // Drafts of the text fields, saved when a field loses focus; everything else applies immediately.
  let drafts = $state({
    claudePath: app.settings.claudePath,
    claudeModel: app.settings.claudeModel,
    opencodePath: app.settings.opencodePath,
    opencodeModel: app.settings.opencodeModel,
  });
  let check = $state<{ ok: boolean; text: string } | null>(null);
  let checking = $state(false);

  const close = () => (app.settingsOpen = false);

  async function saveText() {
    const changed = (Object.keys(drafts) as (keyof typeof drafts)[]).some(
      (k) => drafts[k].trim() !== app.settings[k],
    );
    if (!changed) return;
    check = null;
    const trimmed = Object.fromEntries(Object.entries(drafts).map(([k, v]) => [k, v.trim()]));
    await app.updateSettings(trimmed);
  }

  async function selectAgent(a: Agent) {
    check = null;
    await app.updateSettings({ agent: a.id });
  }

  async function browse() {
    const picked = await open({
      title: `Choose the ${agent.exe} executable`,
      defaultPath: drafts[agent.pathKey] || undefined,
    });
    if (typeof picked === "string") {
      drafts[agent.pathKey] = picked;
      await saveText();
    }
  }

  async function runCheck() {
    checking = true;
    try {
      check = { ok: true, text: await commands.checkAgent(agent.id, drafts[agent.pathKey]) };
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
        {#each AGENTS as a (a.id)}
          <label
            class="flex flex-1 items-center gap-2 rounded-lg border-2 bg-raised px-3 py-2 text-sm"
            class:border-accent={agent.id === a.id}
            class:border-transparent={agent.id !== a.id}
          >
            <input
              type="radio"
              name="agent"
              value={a.id}
              checked={agent.id === a.id}
              onchange={() => selectAgent(a)}
            />
            {a.label}
          </label>
        {/each}
      </div>
      <p class="text-xs text-muted">
        Used for new agent sessions. Tickets that already have a session keep their agent.
      </p>

      <label class="block">
        <span class="mb-1 block text-xs font-medium text-muted">{agent.label} executable</span>
        <div class="flex gap-2">
          <input
            class="min-w-0 flex-1 rounded-md border border-line bg-raised px-3 py-2 font-mono text-sm outline-none focus:border-accent"
            bind:value={drafts[agent.pathKey]}
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
          bind:value={drafts[agent.modelKey]}
          onblur={saveText}
          placeholder={agent.modelHint}
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
