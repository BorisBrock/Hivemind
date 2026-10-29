<script lang="ts">
  import { markdown } from "@codemirror/lang-markdown";
  import { HighlightStyle, syntaxHighlighting } from "@codemirror/language";
  import { EditorView, minimalSetup } from "codemirror";
  import { keymap, placeholder as placeholderExt } from "@codemirror/view";
  import { indentWithTab } from "@codemirror/commands";
  import { tags } from "@lezer/highlight";
  import { onMount } from "svelte";

  let {
    value = $bindable(""),
    placeholder = "",
    autofocus = false,
  }: { value?: string; placeholder?: string; autofocus?: boolean } = $props();

  let host: HTMLDivElement;

  // Styled markdown source: headings, emphasis and code look like their rendered form.
  const highlight = HighlightStyle.define([
    { tag: tags.heading1, fontSize: "1.6em", fontWeight: "700" },
    { tag: tags.heading2, fontSize: "1.35em", fontWeight: "700" },
    { tag: tags.heading3, fontSize: "1.15em", fontWeight: "600" },
    { tag: [tags.heading4, tags.heading5, tags.heading6], fontWeight: "600" },
    { tag: tags.strong, fontWeight: "700" },
    { tag: tags.emphasis, fontStyle: "italic" },
    { tag: tags.strikethrough, textDecoration: "line-through" },
    { tag: [tags.link, tags.url], color: "var(--wz-accent)" },
    { tag: tags.monospace, fontFamily: "var(--font-mono)", color: "var(--wz-accent)" },
    { tag: tags.quote, color: "var(--wz-muted)", fontStyle: "italic" },
    { tag: [tags.processingInstruction, tags.meta, tags.contentSeparator], color: "var(--wz-muted)" },
  ]);

  const theme = EditorView.theme({
    "&": { height: "100%", fontSize: "15px", color: "var(--wz-fg)", backgroundColor: "transparent" },
    "&.cm-focused": { outline: "none" },
    ".cm-scroller": { fontFamily: "var(--font-sans)", lineHeight: "1.65" },
    ".cm-content": { padding: "12px 4px", caretColor: "var(--wz-accent)" },
    ".cm-cursor": { borderLeftColor: "var(--wz-accent)", borderLeftWidth: "2px" },
    ".cm-selectionBackground, &.cm-focused .cm-selectionBackground, ::selection": {
      backgroundColor: "color-mix(in srgb, var(--wz-accent) 30%, transparent) !important",
    },
    ".cm-placeholder": { color: "var(--wz-muted)" },
  });

  onMount(() => {
    const view = new EditorView({
      parent: host,
      doc: value,
      extensions: [
        minimalSetup,
        keymap.of([indentWithTab]),
        markdown(),
        syntaxHighlighting(highlight),
        theme,
        EditorView.lineWrapping,
        placeholderExt(placeholder),
        EditorView.updateListener.of((u) => {
          if (u.docChanged) value = u.state.doc.toString();
        }),
      ],
    });
    if (autofocus) view.focus();
    return () => view.destroy();
  });
</script>

<div bind:this={host} class="h-full min-h-0"></div>
