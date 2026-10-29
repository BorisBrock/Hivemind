<script lang="ts">
  import MarkdownIt from "markdown-it";

  let { source }: { source: string } = $props();

  // Raw HTML is disabled, so rendering with {@html} cannot inject markup.
  const md = new MarkdownIt({ html: false, linkify: true, breaks: true });
  const rendered = $derived(md.render(source));
</script>

<div class="markdown">
  {@html rendered}
</div>
