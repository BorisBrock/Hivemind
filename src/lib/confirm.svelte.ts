/** Themed replacement for the native `ask()` message box. */

/** Picks the icon and its colour. */
export type ConfirmKind = "question" | "info" | "warning" | "error";

export interface ConfirmOptions {
  title: string;
  /** Defaults to "warning" for `danger` dialogs, otherwise "question". */
  kind?: ConfirmKind;
  message: string;
  okLabel?: string;
  cancelLabel?: string;
  /** Styles the confirm button as destructive. */
  danger?: boolean;
}

export interface ConfirmRequest extends ConfirmOptions {
  resolve: (confirmed: boolean) => void;
}

class ConfirmState {
  // Raw, not deep: a proxy would break the identity check in `confirm()`.
  current = $state.raw<ConfirmRequest | null>(null);
}

export const confirmState = new ConfirmState();

/** Shows a confirmation dialog in the app's theme; resolves true if confirmed. */
export function confirm(options: ConfirmOptions): Promise<boolean> {
  // Only one at a time: a newer request cancels a pending one.
  confirmState.current?.resolve(false);
  return new Promise((resolve) => {
    const request: ConfirmRequest = {
      ...options,
      resolve: (confirmed) => {
        if (confirmState.current === request) confirmState.current = null;
        resolve(confirmed);
      },
    };
    confirmState.current = request;
  });
}
