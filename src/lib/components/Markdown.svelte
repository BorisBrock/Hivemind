<script lang="ts">
  import { mount, unmount } from "svelte";
  import MarkdownIt from "markdown-it";
  import { app } from "$lib/app.svelte";
  import { imageUrl } from "$lib/attachments";
  import type { LocalFile } from "$lib/bindings";
  import { type FileEnv, fileLinks } from "$lib/fileLinks";
  import CopyButton from "./CopyButton.svelte";
  import Icon from "./Icon.svelte";
  import Modal from "./Modal.svelte";

  let {
    source,
    cwd,
  }: {
    source: string;
    /** Folder of the agent that wrote the text. If set, files it mentions get links and previews. */
    cwd?: string;
  } = $props();

  // Raw HTML is disabled, so rendering with {@html} cannot inject markup.
  const md = new MarkdownIt({ html: false, linkify: true, breaks: true }).use(fileLinks);
  // Otherwise "main.rs" and "README.md" become web links: .rs and .md are country domains.
  md.linkify.set({ fuzzyLink: false });
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

  const mentions = $derived.by(() => {
    if (!cwd) return [];
    const found = new Set<string>();
    md.parse(source, { mentions: found } satisfies FileEnv);
    return [...found];
  });
  let files = $state.raw(new Map<string, LocalFile>());
  $effect(() => {
    let current = true;
    const dir = cwd;
    if (dir && mentions.length) {
      app.resolveFiles(dir, mentions).then((found) => {
        if (current) files = new Map(found.map((file) => [file.mention, file]));
      });
    }
    return () => (current = false);
  });

  const rendered = $derived.by(() => {
    const env: FileEnv = cwd ? { files, inlineImages: new Set() } : {};
    const html = md.render(source, env);
    const images = new Map([...files.values()].filter((f) => f.isImage).map((f) => [f.path, f]));
    // A Markdown image already shows in the text.
    for (const path of env.inlineImages ?? []) images.delete(path);
    return { html, previews: [...images.values()] };
  });

  let container: HTMLDivElement;
  $effect(() => {
    void rendered;
    const mounted = [
      ...[...container.querySelectorAll(".code-block")].map((block) => {
        const text = block.querySelector("pre")!.textContent.replace(/\n$/, "");
        return mount(CopyButton, { target: block, props: { text } });
      }),
      ...[...container.querySelectorAll(".file-reveal")].map((target) =>
        mount(Icon, { target, props: { name: "folder", size: 13 } }),
      ),
    ];
    return () => mounted.forEach((part) => unmount(part));
  });

  /** Links must never navigate: that would replace the whole app in its window. */
  function onclick(e: MouseEvent) {
    const target = e.target as Element;
    const reveal = target.closest<HTMLElement>("[data-reveal]");
    if (reveal) {
      app.revealFile(reveal.dataset.reveal!);
      return;
    }
    const link = target.closest("a");
    if (!link) return;
    e.preventDefault();
    const href = link.getAttribute("href") ?? "";
    if (link.dataset.file) app.openFile(link.dataset.file);
    else if (/^(https?:|mailto:)/i.test(href)) app.openWebLink(href);
  }

  const fileName = (path: string) => path.slice(path.lastIndexOf("/") + 1);

  let viewing = $state<LocalFile | null>(null);
</script>

<!-- Only a catch-all for the links and buttons inside, which handle the keyboard themselves. -->
<!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
<div bind:this={container} class="markdown" {onclick}>
  {@html rendered.html}
</div>

{#if rendered.previews.length}
  <div class="mt-2 flex flex-wrap gap-2">
    {#each rendered.previews as file (file.path)}
      <button class="block cursor-zoom-in" title="Preview {file.path}" onclick={() => (viewing = file)}>
        <img
          src={imageUrl(file.path)}
          alt={fileName(file.path)}
          class="image-preview h-28 max-w-64 rounded border border-line object-contain"
        />
      </button>
    {/each}
  </div>
{/if}

{#if viewing}
  {@const file = viewing}
  <Modal title={fileName(file.path)} wide onclose={() => (viewing = null)}>
    <img src={imageUrl(file.path)} alt={fileName(file.path)} class="image-preview size-full object-contain" />
    {#snippet footer()}
      <span class="min-w-0 flex-1 truncate font-mono text-xs text-muted" title={file.path}>{file.path}</span>
      <button
        class="rounded-md border border-line px-3 py-1.5 text-sm hover:bg-raised"
        onclick={() => app.revealFile(file.path)}
      >
        Show in file manager
      </button>
      <button
        class="rounded-md border border-line px-3 py-1.5 text-sm hover:bg-raised"
        onclick={() => app.openFile(file.path)}
      >
        Open
      </button>
    {/snippet}
  </Modal>
{/if}
