<script lang="ts">
  import Icon from "./Icon.svelte";

  /** Copies `text` to the clipboard; sits in the corner of a `.code-block`. */
  let { text }: { text: string } = $props();

  let result = $state<"copied" | "failed">();
  let timer: ReturnType<typeof setTimeout>;

  async function copy() {
    try {
      await navigator.clipboard.writeText(text);
      result = "copied";
    } catch {
      result = "failed";
    }
    clearTimeout(timer);
    timer = setTimeout(() => (result = undefined), 1500);
  }

  const label = $derived(result === "copied" ? "Copied" : result === "failed" ? "Copy failed" : "Copy");
</script>

<button
  type="button"
  class="copy-button"
  class:shown={result}
  title={label}
  aria-label={label}
  onclick={copy}
>
  <Icon name={result === "copied" ? "check" : result === "failed" ? "circle-x" : "copy"} size={14} />
</button>
