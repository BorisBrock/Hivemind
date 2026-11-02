<script lang="ts">
  import { mount, unmount } from "svelte";
  import MarkdownIt from "markdown-it";
  import { imageUrl } from "$lib/attachments";
  import CopyButton from "./CopyButton.svelte";

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
  // Code blocks get a wrapper, so their copy button stays in place when the code scrolls.
  for (const rule of ["fence", "code_block"] as const) {
    const renderCode = md.renderer.rules[rule]!;
    md.renderer.rules[rule] = (...args) => `<div class="code-block">${renderCode(...args)}</div>`;
  }
  const rendered = $derived(md.render(source));

  let container: HTMLDivElement;
  $effect(() => {
    void rendered;
    const buttons = [...container.querySelectorAll(".code-block")].map((block) => {
      const text = block.querySelector("pre")!.textContent.replace(/\n$/, "");
      return mount(CopyButton, { target: block, props: { text } });
    });
    return () => buttons.forEach((button) => unmount(button));
  });
</script>

<div bind:this={container} class="markdown">
  {@html rendered}
</div>
