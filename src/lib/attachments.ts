import { convertFileSrc } from "@tauri-apps/api/core";
import { app } from "./app.svelte";

/** Image types the agents accept, by MIME type; mirrors `IMAGE_TYPES` in the backend. */
const EXTENSIONS: Record<string, string> = {
  "image/png": "png",
  "image/jpeg": "jpg",
  "image/gif": "gif",
  "image/webp": "webp",
};

/** Absolute local path (`/…` or `C:/…`), as used for pasted images. */
const isLocalPath = (src: string) => /^([a-zA-Z]:)?[\/]/.test(src);

/** URL to show an image; local files go through Tauri's asset protocol. */
export const imageUrl = (src: string) => (isLocalPath(src) ? convertFileSrc(src) : src);

/** Local image paths a text links to, like the backend's `referenced_images`. */
export function referencedImages(text: string): string[] {
  const paths = [...text.matchAll(/!\[[^\]]*\]\((?:<([^>]*)>|([^)\s]*))/g)]
    .map((m) => m[1] ?? m[2])
    .filter((p) => isLocalPath(p) && /\.(png|jpe?g|gif|webp)$/i.test(p));
  return [...new Set(paths)];
}

/**
 * Saves the images of a paste event and returns Markdown that links them, or null if the
 * clipboard holds no image (then the paste should go ahead as usual). Call synchronously from
 * the event handler: the clipboard is only readable during the event.
 */
export function pastedImages(e: ClipboardEvent): Promise<string> | null {
  const files = [...(e.clipboardData?.files ?? [])].filter((f) => f.type in EXTENSIONS);
  if (!files.length) return null;
  e.preventDefault();
  return Promise.all(
    files.map(async (file) => {
      const path = await app.saveAttachment(await toBase64(file), EXTENSIONS[file.type]);
      return path ? `![image](<${path}>)` : "";
    }),
  ).then((links) => links.filter(Boolean).join("\n"));
}

function toBase64(file: Blob): Promise<string> {
  return new Promise((resolve, reject) => {
    const reader = new FileReader();
    reader.onload = () => resolve((reader.result as string).split(",", 2)[1]);
    reader.onerror = () => reject(reader.error);
    reader.readAsDataURL(file);
  });
}
