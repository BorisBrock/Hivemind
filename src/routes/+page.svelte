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
</script>

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
