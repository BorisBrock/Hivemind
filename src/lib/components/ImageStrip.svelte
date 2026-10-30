<script lang="ts">
  import { imageUrl, referencedImages } from "$lib/attachments";

  /** Thumbnails of the images a Markdown text links, for inputs that show the raw text. */
  let { text }: { text: string } = $props();

  const images = $derived(referencedImages(text));
</script>

{#if images.length}
  <div class="flex shrink-0 gap-2 overflow-x-auto">
    {#each images as path (path)}
      <img
        src={imageUrl(path)}
        alt={path}
        title={path}
        class="h-16 shrink-0 rounded border border-line bg-raised object-contain"
      />
    {/each}
  </div>
{/if}
