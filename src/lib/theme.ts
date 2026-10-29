/** Options offered for the settings that the CSS in app.css understands. */
export const THEMES = [
  { id: "midnight", label: "Midnight" },
  { id: "daylight", label: "Daylight" },
  { id: "nord", label: "Nord" },
] as const;

export const PALETTES = [
  { id: "vivid", label: "Vivid" },
  { id: "pastel", label: "Pastel" },
] as const;

export function applyAppearance(theme: string, palette: string) {
  const root = document.documentElement;
  root.dataset.theme = theme;
  root.dataset.palette = palette;
}

/** CSS colour of a project's palette slot. */
export function projectColor(colorIndex: number) {
  return `var(--p-${colorIndex})`;
}
