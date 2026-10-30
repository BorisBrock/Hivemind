<script lang="ts">
  // Shows a file path on one line. When space runs out, the middle is elided so that
  // both the beginning and the last folder stay visible ("D:\dev\Pro…\hivemind").
  let { path, class: className = "" }: { path: string; class?: string } = $props();

  const parts = $derived.by(() => {
    const trimmed = path.replace(/[\\/]+$/, "");
    const cut = Math.max(trimmed.lastIndexOf("/"), trimmed.lastIndexOf("\\"));
    if (cut <= 0) return { head: "", tail: path };
    return { head: trimmed.slice(0, cut + 1), tail: trimmed.slice(cut + 1) };
  });
</script>

<span class="flex min-w-0 whitespace-nowrap {className}" title={path}>
  {#if parts.head}
    <!-- The head absorbs nearly all shrinking; the tail only shrinks once the head is at its minimum. -->
    <span class={["shrink-[1000] truncate", parts.head.length > 6 && "min-w-[6ch]"]}>{parts.head}</span>
  {/if}
  <span class="min-w-0 truncate">{parts.tail}</span>
</span>
