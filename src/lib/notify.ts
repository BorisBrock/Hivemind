import { getCurrentWindow, UserAttentionType } from "@tauri-apps/api/window";
import { isPermissionGranted, requestPermission, sendNotification } from "@tauri-apps/plugin-notification";
import type { Ticket } from "./app.svelte";

let permitted: boolean | undefined;

/** Tells the user that an agent finished or needs them, unless they are looking at Workzone. */
export async function notifyAgentStopped(ticket: Ticket, projectName: string) {
  const window = getCurrentWindow();
  if (await window.isFocused()) return;
  const waiting = ticket.status === "needs_input";
  await window.requestUserAttention(waiting ? UserAttentionType.Critical : UserAttentionType.Informational);

  permitted ??= (await isPermissionGranted()) || (await requestPermission()) === "granted";
  if (!permitted) return;
  sendNotification({
    title: `${projectName}: ${ticket.title}`,
    body: waiting ? "The agent needs your input." : "The agent has finished.",
  });
}
