<script lang="ts">
  import type { ConfirmKind, ConfirmRequest } from "$lib/confirm.svelte";
  import Icon from "./Icon.svelte";

  let { request }: { request: ConfirmRequest } = $props();

  const kinds = {
    question: { icon: "circle-help", color: "var(--color-accent)" },
    info: { icon: "info", color: "var(--color-accent)" },
    warning: { icon: "triangle-alert", color: "var(--color-warning)" },
    error: { icon: "circle-x", color: "var(--color-danger)" },
  } as const satisfies Record<ConfirmKind, unknown>;

  const kind = $derived(kinds[request.kind ?? (request.danger ? "warning" : "question")]);

  let dialog: HTMLDialogElement;
  let pressedOnBackdrop = false;
  $effect(() => {
    dialog.showModal();
    dialog.querySelector<HTMLElement>("[data-autofocus]")?.focus();
  });
</script>

<!-- Own <dialog> rather than Modal: a message box has no header bar or ✕, just icon, text and buttons. -->
<dialog
  bind:this={dialog}
  onclose={() => request.resolve(false)}
  onpointerdown={(e) => (pressedOnBackdrop = e.target === dialog)}
  onclick={(e) => {
    // Ignore drags that start inside the dialog and end on the backdrop.
    if (e.target === dialog && pressedOnBackdrop) dialog.close();
    pressedOnBackdrop = false;
  }}
  class="m-auto w-[min(460px,92vw)] overflow-hidden rounded-2xl border border-line bg-surface p-0 text-fg shadow-2xl backdrop:bg-black/50 backdrop:backdrop-blur-[2px]"
>
  <div class="flex gap-4 px-6 pt-6 pb-5">
    <div
      class="grid size-11 shrink-0 place-items-center rounded-full"
      style:color={kind.color}
      style:background="color-mix(in srgb, {kind.color} 15%, transparent)"
    >
      <Icon name={kind.icon} size={22} />
    </div>
    <div class="min-w-0 pt-0.5">
      <h2 class="text-base font-semibold">{request.title}</h2>
      <p class="mt-1.5 text-sm leading-relaxed whitespace-pre-line text-muted select-text">
        {request.message}
      </p>
    </div>
  </div>

  <footer class="flex justify-end gap-2 border-t border-line bg-raised px-6 py-4">
    <button
      class="rounded-lg border border-line px-4 py-2 hover:bg-surface"
      onclick={() => request.resolve(false)}
    >
      {request.cancelLabel ?? "Cancel"}
    </button>
    <button
      class="rounded-lg px-4 py-2 font-medium text-bg hover:brightness-110"
      class:bg-accent={!request.danger}
      class:bg-danger={request.danger}
      data-autofocus
      onclick={() => request.resolve(true)}
    >
      {request.okLabel ?? "OK"}
    </button>
  </footer>
</dialog>
