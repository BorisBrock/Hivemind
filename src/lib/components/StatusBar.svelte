<script lang="ts">
  import { app } from "$lib/app.svelte";
  import { PALETTES, THEMES } from "$lib/theme";
  import Icon from "./Icon.svelte";

  // Filled in by the agent runner in Phase 3.
  const runningAgents = 0;
</script>

<footer class="flex h-7 shrink-0 items-center gap-4 border-t border-line bg-surface px-3 text-xs text-muted">
  <span class="flex items-center gap-1.5" title="Active agent sessions">
    <Icon name="bot" size={13} />
    {runningAgents} running
  </span>
  <span title="Token usage (available once agents run)">Tokens: —</span>
  {#if app.error}
    <span class="truncate text-danger" title={app.error}>{app.error}</span>
  {/if}

  <!-- Temporary home for appearance options until the settings dialog (Phase 4). -->
  <div class="ml-auto flex items-center gap-2">
    <select
      class="rounded bg-raised px-1 py-0.5 text-fg"
      value={app.settings.theme}
      onchange={(e) => app.updateSettings({ theme: e.currentTarget.value })}
    >
      {#each THEMES as t (t.id)}<option value={t.id}>{t.label}</option>{/each}
    </select>
    <select
      class="rounded bg-raised px-1 py-0.5 text-fg"
      value={app.settings.palette}
      onchange={(e) => app.updateSettings({ palette: e.currentTarget.value })}
    >
      {#each PALETTES as p (p.id)}<option value={p.id}>{p.label}</option>{/each}
    </select>
  </div>
</footer>
