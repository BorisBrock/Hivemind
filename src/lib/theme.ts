/** Options offered for the settings that the CSS in app.css understands. */
export const THEMES = [
  { id: "midnight", label: "Midnight", dark: true },
  { id: "nord", label: "Nord", dark: true },
  { id: "dracula", label: "Dracula", dark: true },
  { id: "gruvbox", label: "Gruvbox", dark: true },
  { id: "catppuccin-mocha", label: "Catppuccin Mocha", dark: true },
  { id: "tokyo-night", label: "Tokyo Night", dark: true },
  { id: "solarized-dark", label: "Solarized Dark", dark: true },
  { id: "high-contrast", label: "High Contrast", dark: true },
  { id: "daylight", label: "Daylight", dark: false },
  { id: "solarized-light", label: "Solarized Light", dark: false },
  { id: "catppuccin-latte", label: "Catppuccin Latte", dark: false },
  { id: "paper", label: "Paper", dark: false },
] as const;

export const PALETTES = [
  { id: "vivid", label: "Vivid" },
  { id: "pastel", label: "Pastel" },
  { id: "earth", label: "Earth" },
  { id: "neon", label: "Neon" },
  { id: "colorblind", label: "Colour-blind safe" },
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
