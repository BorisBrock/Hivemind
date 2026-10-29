<script lang="ts">
  import { onMount } from "svelte";
  import { app } from "$lib/app.svelte";
  import Board from "$lib/components/Board.svelte";
  import ProjectDialog from "$lib/components/ProjectDialog.svelte";
  import SettingsDialog from "$lib/components/SettingsDialog.svelte";
  import Sidebar from "$lib/components/Sidebar.svelte";
  import StatusBar from "$lib/components/StatusBar.svelte";
  import TicketDialog from "$lib/components/TicketDialog.svelte";

  onMount(() => app.load());

  const dialogOpen = $derived(!!(app.settingsOpen || app.projectDialog || app.ticketDialog));

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

<div class="flex h-screen flex-col">
  <div class="flex min-h-0 flex-1">
    <Sidebar />
    <Board />
  </div>
  <StatusBar />
</div>

{#if app.settingsOpen}
  <SettingsDialog />
{/if}
{#if app.projectDialog}
  <ProjectDialog dialog={app.projectDialog} />
{/if}
{#if app.ticketDialog}
  {#key app.ticketDialog}
    <TicketDialog dialog={app.ticketDialog} />
  {/key}
{/if}
