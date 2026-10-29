<script lang="ts">
  import type { Snippet } from "svelte";

  let {
    title,
    onclose,
    wide = false,
    accent,
    children,
    footer,
  }: {
    title: string;
    onclose: () => void;
    wide?: boolean;
    /** CSS colour for a thin strip along the top edge (e.g. the ticket's project colour). */
    accent?: string;
    children: Snippet;
    footer?: Snippet;
  } = $props();

  let dialog: HTMLDialogElement;
  $effect(() => dialog.showModal());
</script>

<!-- Native <dialog>: focus trapping and Escape-to-close come for free. -->
<dialog
  bind:this={dialog}
  {onclose}
  onclick={(e) => e.target === dialog && dialog.close()}
  class="m-auto flex max-h-[90vh] flex-col overflow-hidden rounded-xl border border-line bg-surface p-0 text-fg shadow-2xl backdrop:bg-black/50 backdrop:backdrop-blur-[2px]"
  class:w-[min(1100px,92vw)]={wide}
  class:h-[85vh]={wide}
  class:w-[min(520px,92vw)]={!wide}
>
  {#if accent}
    <div class="h-1 shrink-0 transition-colors" style:background={accent}></div>
  {/if}
  <header class="flex items-center justify-between border-b border-line px-5 py-3">
    <h2 class="text-base font-semibold">{title}</h2>
    <button
      class="rounded p-1 text-muted hover:bg-raised hover:text-fg"
      onclick={() => dialog.close()}
      title="Close"
    >
      ✕
    </button>
  </header>
  <div class="min-h-0 flex-1 overflow-y-auto p-5 select-text">
    {@render children()}
  </div>
  {#if footer}
    <footer class="flex items-center gap-2 border-t border-line px-5 py-3">
      {@render footer()}
    </footer>
  {/if}
</dialog>
