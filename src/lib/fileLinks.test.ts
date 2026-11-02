import MarkdownIt from "markdown-it";
import { describe, expect, it } from "vitest";
import type { LocalFile } from "./bindings";
import { type FileEnv, fileLinks, pathsInText } from "./fileLinks";

const md = new MarkdownIt({ html: false, linkify: true, breaks: true }).use(fileLinks);
md.linkify.set({ fuzzyLink: false });

const mentions = (source: string) => {
  const env: FileEnv = { mentions: new Set() };
  md.parse(source, env);
  return [...env.mentions!];
};

const file = (mention: string, extra: Partial<LocalFile> = {}): LocalFile => ({
  mention,
  path: mention.startsWith("/") ? mention : `/repo/${mention}`,
  isImage: false,
  canOpen: true,
  ...extra,
});

const render = (source: string, ...found: LocalFile[]) => {
  const env: FileEnv = { files: new Map(found.map((f) => [f.mention, f])), inlineImages: new Set() };
  return { html: md.render(source, env), inlineImages: [...env.inlineImages!] };
};

describe("finding paths", () => {
  it.each([
    ["SVG: /home/me/git/game/yellow_circle.svg", "/home/me/git/game/yellow_circle.svg"],
    ["See yellow_circle.svg.", "yellow_circle.svg"],
    ["Changed src/lib/app.ts:12, see above", "src/lib/app.ts"],
    ["Changed src/lib/app.ts:12:5", "src/lib/app.ts"],
    ["In src/a.rs#L10-L20", "src/a.rs"],
    ["Open C:\\dev\\game\\a.png now", "C:\\dev\\game\\a.png"],
    ["Open C:/dev/game/a.png now", "C:/dev/game/a.png"],
    ["Look in ~/shots/a.png", "~/shots/a.png"],
    ["Look in ../shots/a.png", "../shots/a.png"],
    ["(/tmp/a.png)", "/tmp/a.png"],
    ["src/Ünï/bïld.png", "src/Ünï/bïld.png"],
  ])("in text: %s", (text, path) => {
    expect(pathsInText(text).map((m) => m[2])).toEqual([path]);
  });

  it("ignores plain words and numbers", () => {
    expect(pathsInText("Version 1.2.3 is out, it works well, sir")).toEqual([]);
  });

  it("finds paths in all Markdown forms", () => {
    const source = [
      "Plain yellow_circle.svg, code `a b/c d.png`, link [the file](</tmp/x y.pdf>)",
      "and image ![pic](shot.png)",
    ].join("\n");
    expect(mentions(source).sort()).toEqual(
      ["a b/c d.png", "shot.png", "/tmp/x y.pdf", "yellow_circle.svg"].sort(),
    );
  });

  it("leaves web links and fenced code alone", () => {
    expect(
      mentions("https://example.com/a.png [b](https://example.com/b.png)\n\n```\n/tmp/c.png\n```"),
    ).toEqual([]);
  });

  it("does not mistake file names for web addresses", () => {
    expect(md.render("See README.md and main.rs")).not.toContain("<a ");
  });
});

describe("linking files", () => {
  it("links existing files only, keeping the text around them", () => {
    const { html } = render("Made /tmp/a.svg, not /tmp/b.svg.", file("/tmp/a.svg"));
    expect(html).toContain('Made <a href="#" data-file="/tmp/a.svg"');
    expect(html).toContain(">/tmp/a.svg</a>");
    expect(html).toContain(", not /tmp/b.svg.");
    expect(html).toContain('data-reveal="/tmp/a.svg"');
  });

  it("links several files in one text", () => {
    const { html } = render("a.png and b.png", file("a.png"), file("b.png"));
    expect(html.match(/data-file=/g)).toHaveLength(2);
    expect(html).toContain("</button> and <a");
  });

  it("links inline code, with the code inside the link", () => {
    const { html } = render("See `a.png:3`", file("a.png"));
    expect(html).toContain('data-file="/repo/a.png"');
    expect(html).toContain("<code>a.png:3</code></a>");
  });

  it("points Markdown links at the file", () => {
    const { html } = render("[the report](report.pdf)", file("report.pdf"));
    expect(html).toContain('<a href="#" data-file="/repo/report.pdf"');
    expect(html).toContain(">the report</a>");
  });

  it("offers no second action for files that only show in the file manager", () => {
    const { html } = render("Run go.sh", file("go.sh", { canOpen: false }));
    expect(html).toContain('title="Show /repo/go.sh in file manager"');
    expect(html).not.toContain("data-reveal");
  });

  it("escapes paths", () => {
    const { html } = render('`a"<b>.png`', file('a"<b>.png', { path: '/repo/a"<b>.png' }));
    expect(html).toContain("/repo/a&quot;&lt;b&gt;.png");
    expect(html).not.toContain("<b>");
  });

  it("shows Markdown images by their resolved path and notes them", () => {
    const { html, inlineImages } = render("![pic](shot.png)", file("shot.png", { isImage: true }));
    expect(html).toContain('src="/repo/shot.png"');
    expect(inlineImages).toEqual(["/repo/shot.png"]);
  });

  it("drops the target of images that are not there", () => {
    const { html } = render("![pic](/tmp/gone.png) ![web](https://example.com/a.png)");
    expect(html).not.toContain("gone.png");
    expect(html).toContain('src="https://example.com/a.png"');
  });

  it("changes nothing without a file environment", () => {
    expect(md.render("![pic](/tmp/a.png) /tmp/b.png", {})).toContain('src="/tmp/a.png"');
    expect(md.render("/tmp/b.png", {})).not.toContain("<a");
  });
});
