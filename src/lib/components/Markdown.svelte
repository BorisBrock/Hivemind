<script lang="ts">
  import MarkdownIt from "markdown-it";
  import { imageUrl } from "$lib/attachments";

  let { source }: { source: string } = $props();

  // Raw HTML is disabled, so rendering with {@html} cannot inject markup.
  const md = new MarkdownIt({ html: false, linkify: true, breaks: true });
  // Pasted images link local files, which the webview can only load through the asset protocol.
  const renderImage = md.renderer.rules.image!;
  md.renderer.rules.image = (tokens, idx, options, env, self) => {
    const token = tokens[idx];
    const src = md.normalizeLinkText(String(token.attrGet("src") ?? ""));
    const url = imageUrl(src);
    if (url !== src) token.attrSet("src", url);
    return renderImage(tokens, idx, options, env, self);
  };
  const rendered = $derived(md.render(source));
</script>

<div class="markdown">
  {@html rendered}
</div>
