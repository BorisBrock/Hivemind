<script lang="ts">
  import { onMount } from "svelte";
  import { app } from "$lib/app.svelte";
  import { confirmState } from "$lib/confirm.svelte";
  import Board from "$lib/components/Board.svelte";
  import ConfirmDialog from "$lib/components/ConfirmDialog.svelte";
  import GroupDialog from "$lib/components/GroupDialog.svelte";
  import ProjectDialog from "$lib/components/ProjectDialog.svelte";
  import SettingsDialog from "$lib/components/SettingsDialog.svelte";
  import Sidebar from "$lib/components/Sidebar.svelte";
  import StatusBar from "$lib/components/StatusBar.svelte";
  import TicketDialog from "$lib/components/TicketDialog.svelte";
  import TrashDialog from "$lib/components/TrashDialog.svelte";

  onMount(() => app.load());

  // Sidebar width is a per-machine layout preference, so it lives in localStorage.
  const SIDEBAR_KEY = "sidebarWidth";
  const SIDEBAR_DEFAULT = 288;
  const SIDEBAR_MIN = 200;
  const SIDEBAR_MAX = 640;
  const clampWidth = (w: number) => Math.round(Math.min(SIDEBAR_MAX, Math.max(SIDEBAR_MIN, w)));

  let sidebarWidth = $state(clampWidth(Number(localStorage.getItem(SIDEBAR_KEY)) || SIDEBAR_DEFAULT));
  let resizing = $state(false);

  function setSidebarWidth(w: number) {
    sidebarWidth = clampWidth(w);
    localStorage.setItem(SIDEBAR_KEY, String(sidebarWidth));
  }

  function onSplitterDown(e: PointerEvent) {
    if (e.button !== 0) return;
    e.preventDefault();
    (e.currentTarget as HTMLElement).setPointerCapture(e.pointerId);
    resizing = true;
  }

  function onSplitterMove(e: PointerEvent) {
    if (resizing) setSidebarWidth(e.clientX);
  }

  function onSplitterKey(e: KeyboardEvent) {
    const step = e.shiftKey ? 64 : 16;
    if (e.key === "ArrowLeft") setSidebarWidth(sidebarWidth - step);
    else if (e.key === "ArrowRight") setSidebarWidth(sidebarWidth + step);
    else if (e.key === "Home") setSidebarWidth(SIDEBAR_MIN);
    else if (e.key === "End") setSidebarWidth(SIDEBAR_MAX);
    else return;
    e.preventDefault();
  }

  const dialogOpen = $derived(
    !!(app.settingsOpen || app.trashOpen || app.projectDialog || app.groupDialog || app.ticketDialog),
  );

  /** Global shortcuts; plain keys only when not typing somewhere. */
  function onkeydown(e: KeyboardEvent) {
    const typing =
      e.target instanceof Element &&
      e.target.closest("input, textarea, select, [contenteditable=true], .cm-editor");
    const mod = e.ctrlKey || e.metaKey;
    if (mod && e.key === ",") {
      e.preventDefault();
      app.settingsOpen = true;
      return;
    }
    if (dialogOpen) return;
    if ((mod && e.key.toLowerCase() === "k") || (!typing && !mod && e.key === "/")) {
      e.preventDefault();
      document.getElementById("board-search")?.focus();
    } else if (!typing && !mod && !e.altKey && e.key.toLowerCase() === "n") {
      if (app.projects.some((p) => !p.hidden)) {
        e.preventDefault();
        app.ticketDialog = { mode: "create" };
      }
    } else if (!typing && !mod && !e.altKey && e.key.toLowerCase() === "p") {
      e.preventDefault();
      app.projectDialog = { mode: "create" };
    }
  }
</script>

<svelte:window {onkeydown} />

<div class="flex h-screen flex-col" class:select-none={resizing} class:cursor-col-resize={resizing}>
  <div class="flex min-h-0 flex-1">
    <Sidebar width={sidebarWidth} />
    <!-- A focusable separator with a value is an interactive widget per WAI-ARIA. -->
    <!-- svelte-ignore a11y_no_noninteractive_tabindex, a11y_no_noninteractive_element_interactions -->
    <div
      role="separator"
      aria-orientation="vertical"
      aria-label="Resize projects panel"
      aria-valuenow={sidebarWidth}
      aria-valuemin={SIDEBAR_MIN}
      aria-valuemax={SIDEBAR_MAX}
      tabindex="0"
      title="Drag to resize, double-click to reset"
      class="group relative w-px shrink-0 cursor-col-resize bg-line outline-none"
      onpointerdown={onSplitterDown}
      onpointermove={onSplitterMove}
      onpointerup={() => (resizing = false)}
      onpointercancel={() => (resizing = false)}
      ondblclick={() => setSidebarWidth(SIDEBAR_DEFAULT)}
      onkeydown={onSplitterKey}
    >
      <!-- Wider invisible hit area; highlights while hovered, focused or dragging. -->
      <div
        class="absolute inset-y-0 -left-1 -right-1 z-10 transition-colors group-hover:bg-accent/40 group-focus-visible:bg-accent/60"
        class:bg-accent={resizing}
      ></div>
    </div>
    <Board />
  </div>
  <StatusBar />
</div>

{#if app.settingsOpen}
  <SettingsDialog />
{/if}
{#if app.trashOpen}
  <TrashDialog />
{/if}
{#if app.projectDialog}
  <ProjectDialog dialog={app.projectDialog} />
{/if}
{#if app.groupDialog}
  <GroupDialog dialog={app.groupDialog} />
{/if}
{#if app.ticketDialog}
  {#key app.ticketDialog}
    <TicketDialog dialog={app.ticketDialog} />
  {/key}
{/if}
<!-- Last, so it stacks above any dialog that asked for it. -->
{#if confirmState.current}
  {#key confirmState.current}
    <ConfirmDialog request={confirmState.current} />
  {/key}
{/if}
