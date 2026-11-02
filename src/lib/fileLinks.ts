import type MarkdownIt from "markdown-it";
import type { LocalFile } from "./bindings";

/**
 * Environment of a Markdown pass. Parsing with `mentions` collects the paths a text mentions, to
 * ask the backend which of them exist. Rendering with `files` (the answer) links those files.
 * Without either, the text is left as it is.
 */
export type FileEnv = {
  mentions?: Set<string>;
  files?: Map<string, LocalFile>;
  /** Filled while rendering: files the text already shows as Markdown images. */
  inlineImages?: Set<string>;
};

/** Optional drive, `~/` or `./` start, then path segments, with an optional `:line` or `#L1` end. */
const PATH_IN_TEXT =
  /(?<![\p{L}\p{N}_.@+\-\/\\:~])(?:[a-zA-Z]:[\\/]|~[\\/]|\.{0,2}[\\/])?[\p{L}\p{N}_.@+\-]+(?:[\\/][\p{L}\p{N}_.@+\-]+)*(?::\d+(?:[:-]\d+)?|#L\d+(?:-L?\d+)?)?/gu;
const LINE_SUFFIX = /(?::\d+(?:[:-]\d+)?|#L\d+(?:-L?\d+)?)$/;
const HAS_SCHEME = /^[a-z][a-z0-9+.-]+:/i;
/** A slash, or a file name with an extension: "and/or" gets through, plain words do not. */
const LOOKS_LIKE_PATH = /[\\/]|\.\p{L}[\p{L}\p{N}]*$/u;

const withoutLine = (path: string) => path.replace(LINE_SUFFIX, "");

/** The path a whole piece of text stands for, if it may be one. */
function mentionIn(text: string): string | null {
  const path = withoutLine(text.trim());
  return path.length <= 1024 && !path.includes("\n") && LOOKS_LIKE_PATH.test(path) ? path : null;
}

/** Paths in running text, as `[start, end, path]`; `end` may include a line number. */
export function pathsInText(text: string): [number, number, string][] {
  const found: [number, number, string][] = [];
  for (const match of text.matchAll(PATH_IN_TEXT)) {
    // Sentence punctuation is not part of the path.
    const raw = match[0].replace(/[.\-]+$/, "");
    const path = mentionIn(raw);
    if (path) found.push([match.index, match.index + raw.length, path]);
  }
  return found;
}

/** Links the files that a text mentions. See {@link FileEnv} for the two passes. */
export function fileLinks(md: InstanceType<typeof MarkdownIt>) {
  md.renderer.rules.file_reveal = (tokens, idx, _options, _env, self) =>
    `<button type="button" class="file-reveal" title="Show in file manager"${self.renderAttrs(tokens[idx])}></button>`;

  /** The local path that a link or image target stands for. */
  const targetMention = (attribute: string | number | null) => {
    const target = String(attribute ?? "");
    if (!target || target.startsWith("#") || HAS_SCHEME.test(target)) return null;
    return withoutLine(md.normalizeLinkText(target));
  };

  md.core.ruler.after("linkify", "file_links", (state) => {
    const env = state.env as FileEnv;
    const { mentions, files } = env;
    if (!mentions && !files) return;
    /** Whether `path` is to be linked; records it in the first pass. */
    const lookUp = (path: string) => {
      mentions?.add(path);
      return files?.get(path);
    };
    const linkAttrs = (file: LocalFile): [string, string][] => [
      ["href", "#"],
      ["data-file", file.path],
      ["title", file.canOpen ? `Open ${file.path}` : `Show ${file.path} in file manager`],
    ];
    const revealToken = (file: LocalFile) => {
      const token = new state.Token("file_reveal", "", 0);
      token.attrSet("data-reveal", file.path);
      return token;
    };
    /** The tokens of a link to `file` around `inner`. */
    const linked = (file: LocalFile, inner: InstanceType<typeof state.Token>) => {
      const open = new state.Token("link_open", "a", 1);
      open.attrs = linkAttrs(file);
      const out = [open, inner, new state.Token("link_close", "a", -1)];
      return file.canOpen ? [...out, revealToken(file)] : out;
    };

    for (const block of state.tokens) {
      if (block.type !== "inline" || !block.children) continue;
      const out: typeof block.children = [];
      let insideLink = false;
      let linkedFile: LocalFile | undefined;
      for (const token of block.children) {
        if (token.type === "link_open") {
          insideLink = true;
          const mention = targetMention(token.attrGet("href"));
          linkedFile = mention ? lookUp(mention) : undefined;
          if (linkedFile) token.attrs = linkAttrs(linkedFile);
          out.push(token);
        } else if (token.type === "link_close") {
          insideLink = false;
          out.push(token);
          if (linkedFile?.canOpen) out.push(revealToken(linkedFile));
          linkedFile = undefined;
        } else if (token.type === "image") {
          const mention = targetMention(token.attrGet("src"));
          const file = mention ? lookUp(mention) : undefined;
          if (file?.isImage) {
            token.attrSet("src", file.path);
            env.inlineImages?.add(file.path);
          } else if (mention && files) {
            // Not loadable: drop the target so that only the description shows.
            token.attrs = token.attrs!.filter(([name]) => name !== "src");
          }
          out.push(token);
        } else if (token.type === "code_inline" && !insideLink) {
          const mention = mentionIn(token.content);
          const file = mention ? lookUp(mention) : undefined;
          if (file) out.push(...linked(file, token));
          else out.push(token);
        } else if (token.type === "text" && !insideLink) {
          let end = 0;
          for (const [start, stop, path] of pathsInText(token.content)) {
            const file = lookUp(path);
            if (!file) continue;
            const before = new state.Token("text", "", 0);
            before.content = token.content.slice(end, start);
            const label = new state.Token("text", "", 0);
            label.content = token.content.slice(start, stop);
            out.push(before, ...linked(file, label));
            end = stop;
          }
          if (end === 0) {
            out.push(token);
          } else {
            const after = new state.Token("text", "", 0);
            after.content = token.content.slice(end);
            out.push(after);
          }
        } else {
          out.push(token);
        }
      }
      block.children = out;
    }
  });
}
