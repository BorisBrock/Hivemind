<script lang="ts">
  import { app } from "$lib/app.svelte";
  import Icon from "./Icon.svelte";

  const limits = $derived(app.usage?.limits);
  const windows = $derived(
    [
      { label: "Session", hint: "Claude's 5-hour usage window", window: limits?.fiveHour },
      { label: "Week", hint: "Claude's 7-day usage window", window: limits?.sevenDay },
    ].filter((w) => w.window),
  );

  const compact = (n: number) =>
    n >= 1e6 ? `${(n / 1e6).toFixed(1)}M` : n >= 1e3 ? `${(n / 1e3).toFixed(1)}k` : `${n}`;
  const time = (iso?: string | null) =>
    iso
      ? new Date(iso).toLocaleString(undefined, { weekday: "short", hour: "2-digit", minute: "2-digit" })
      : "?";
</script>

<footer class="flex h-7 shrink-0 items-center gap-4 border-t border-line bg-surface px-3 text-xs text-muted">
  <span class="flex items-center gap-1.5" title="Agents working right now">
    <Icon name="bot" size={13} />
    {app.runningCount} running
  </span>

  {#each windows as { label, hint, window } (label)}
    {@const pct = Math.round((window?.utilization ?? 0) * 100)}
    <span
      class="flex items-center gap-1.5"
      title="{hint}: {pct}% used, resets {time(window?.resetsAt)}. Reported {time(limits?.reportedAt)}."
    >
      {label}
      <span class="h-1.5 w-14 overflow-hidden rounded-full bg-raised">
        <span
          class="block h-full rounded-full"
          class:bg-accent={pct < 80}
          class:bg-danger={pct >= 80}
          style:width="{Math.min(pct, 100)}%"
        ></span>
      </span>
      {pct}%
    </span>
  {/each}

  {#if app.usage}
    <span title="Summed over all agent turns of the last 7 days">
      7 days: {compact(app.usage.weekInputTokens + app.usage.weekOutputTokens)} tokens
      {#if app.usage.weekCostUsd}· ${app.usage.weekCostUsd.toFixed(2)}{/if}
    </span>
  {/if}

  {#if app.error}
    <span class="truncate text-danger" title={app.error}>{app.error}</span>
  {/if}

  <button
    class="ml-auto flex items-center gap-1 rounded px-1.5 py-0.5 hover:bg-raised hover:text-fg"
    title="Deleted and closed tickets"
    onclick={() => (app.trashOpen = true)}
  >
    <Icon name="trash" size={13} /> Trash
    {#if app.trash.length}<span class="rounded-full bg-raised px-1.5">{app.trash.length}</span>{/if}
  </button>
  <button
    class="flex items-center gap-1 rounded px-1.5 py-0.5 hover:bg-raised hover:text-fg"
    title="Settings"
    onclick={() => (app.settingsOpen = true)}
  >
    <Icon name="settings" size={13} /> Settings
  </button>
</footer>
