<script lang="ts">
  import { onMount } from "svelte";
  import { app } from "$lib/app.svelte";
  import Board from "$lib/components/Board.svelte";
  import Sidebar from "$lib/components/Sidebar.svelte";
  import StatusBar from "$lib/components/StatusBar.svelte";

  onMount(() => app.load());
</script>

<div class="flex h-screen flex-col">
  <div class="flex min-h-0 flex-1">
    <Sidebar />
    <Board />
  </div>
  <StatusBar />
</div>
