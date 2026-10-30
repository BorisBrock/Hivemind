<script lang="ts">
  import type { ConfirmRequest } from "$lib/confirm.svelte";
  import Modal from "./Modal.svelte";

  let { request }: { request: ConfirmRequest } = $props();
</script>

<Modal title={request.title} onclose={() => request.resolve(false)}>
  <p class="text-sm whitespace-pre-line">{request.message}</p>

  {#snippet footer()}
    <button class="ml-auto rounded-md px-3 py-1.5 hover:bg-raised" onclick={() => request.resolve(false)}>
      {request.cancelLabel ?? "Cancel"}
    </button>
    <button
      class="rounded-md px-4 py-1.5 font-medium text-bg"
      class:bg-accent={!request.danger}
      class:bg-danger={request.danger}
      data-autofocus
      onclick={() => request.resolve(true)}
    >
      {request.okLabel ?? "OK"}
    </button>
  {/snippet}
</Modal>
