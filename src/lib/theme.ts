/**
 * Theme resolution (FR-053, ACC-002/004/005). Chrome colors live in CSS
 * custom properties keyed by `data-theme`; terminal palettes mirror them.
 */
import type { ITheme } from "@xterm/xterm";
import type { Settings, TerminalScheme, Theme } from "./ipc";

export type ResolvedTheme = "dark" | "light" | "high_contrast";

function media(query: string): boolean {
  return typeof window !== "undefined" && typeof window.matchMedia === "function"
    ? window.matchMedia(query).matches
    : false;
}

export function resolveTheme(theme: Theme): ResolvedTheme {
  if (theme !== "system") return theme;
  if (media("(forced-colors: active)") || media("(prefers-contrast: more)")) return "high_contrast";
  return media("(prefers-color-scheme: light)") ? "light" : "dark";
}

export function reducedMotion(settings: Settings | null): boolean {
  const motion = settings?.motion ?? "system";
  if (motion === "reduced") return true;
  if (motion === "full") return false;
  return media("(prefers-reduced-motion: reduce)");
}

export function applyTheme(settings: Settings | null): ResolvedTheme {
  const resolved = resolveTheme(settings?.theme ?? "system");
  const root = document.documentElement;
  root.dataset.theme = resolved;
  root.dataset.motion = reducedMotion(settings) ? "reduced" : "full";
  return resolved;
}

const dark: ITheme = {
  background: "#0f1115",
  foreground: "#e6e8ee",
  cursor: "#7aa2f7",
  cursorAccent: "#0f1115",
  selectionBackground: "#2c3a5c",
  black: "#1b1e26",
  red: "#f7768e",
  green: "#9ece6a",
  yellow: "#e0af68",
  blue: "#7aa2f7",
  magenta: "#bb9af7",
  cyan: "#7dcfff",
  white: "#c0caf5",
  brightBlack: "#565f89",
  brightRed: "#ff8fa3",
  brightGreen: "#b9f27c",
  brightYellow: "#ffc777",
  brightBlue: "#9ab8ff",
  brightMagenta: "#d0b3ff",
  brightCyan: "#a4e2ff",
  brightWhite: "#ffffff",
};

const light: ITheme = {
  background: "#ffffff",
  foreground: "#1f2330",
  cursor: "#2f5bff",
  cursorAccent: "#ffffff",
  selectionBackground: "#c9d7ff",
  black: "#1f2330",
  red: "#c4223b",
  green: "#2e7d32",
  yellow: "#8a6100",
  blue: "#2f5bff",
  magenta: "#8e3bc4",
  cyan: "#00739c",
  white: "#d5d8e0",
  brightBlack: "#5b647a",
  brightRed: "#e0314b",
  brightGreen: "#3a9a40",
  brightYellow: "#a87700",
  brightBlue: "#4a72ff",
  brightMagenta: "#a552d9",
  brightCyan: "#0090c2",
  brightWhite: "#ffffff",
};

const highContrast: ITheme = {
  background: "#000000",
  foreground: "#ffffff",
  cursor: "#ffff00",
  cursorAccent: "#000000",
  selectionBackground: "#0050ff",
  black: "#000000",
  red: "#ff5555",
  green: "#55ff55",
  yellow: "#ffff55",
  blue: "#5599ff",
  magenta: "#ff55ff",
  cyan: "#55ffff",
  white: "#ffffff",
  brightBlack: "#bbbbbb",
  brightRed: "#ff8888",
  brightGreen: "#88ff88",
  brightYellow: "#ffff88",
  brightBlue: "#88bbff",
  brightMagenta: "#ff88ff",
  brightCyan: "#88ffff",
  brightWhite: "#ffffff",
};

/** Named terminal color schemes (Settings → Terminals). */
const pal = (bg: string, fg: string, cursor: string, sel: string, c: string[]): ITheme => ({
  background: bg,
  foreground: fg,
  cursor,
  selectionBackground: sel,
  black: c[0],
  red: c[1],
  green: c[2],
  yellow: c[3],
  blue: c[4],
  magenta: c[5],
  cyan: c[6],
  white: c[7],
  brightBlack: c[8],
  brightRed: c[9],
  brightGreen: c[10],
  brightYellow: c[11],
  brightBlue: c[12],
  brightMagenta: c[13],
  brightCyan: c[14],
  brightWhite: c[15],
});

export const SCHEMES: Record<Exclude<TerminalScheme, "auto">, ITheme> = {
  solarized_dark: pal("#002b36", "#839496", "#93a1a1", "#073642", [
    "#073642",
    "#dc322f",
    "#859900",
    "#b58900",
    "#268bd2",
    "#d33682",
    "#2aa198",
    "#eee8d5",
    "#586e75",
    "#cb4b16",
    "#859900",
    "#b58900",
    "#839496",
    "#6c71c4",
    "#2aa198",
    "#fdf6e3",
  ]),
  solarized_light: pal("#fdf6e3", "#586e75", "#586e75", "#eee8d5", [
    "#073642",
    "#dc322f",
    "#859900",
    "#b58900",
    "#268bd2",
    "#d33682",
    "#2aa198",
    "#eee8d5",
    "#002b36",
    "#cb4b16",
    "#586e75",
    "#657b83",
    "#839496",
    "#6c71c4",
    "#93a1a1",
    "#fdf6e3",
  ]),
  dracula: pal("#282a36", "#f8f8f2", "#f8f8f2", "#44475a", [
    "#21222c",
    "#ff5555",
    "#50fa7b",
    "#f1fa8c",
    "#bd93f9",
    "#ff79c6",
    "#8be9fd",
    "#f8f8f2",
    "#6272a4",
    "#ff6e6e",
    "#69ff94",
    "#ffffa5",
    "#d6acff",
    "#ff92df",
    "#a4ffff",
    "#ffffff",
  ]),
  nord: pal("#2e3440", "#d8dee9", "#d8dee9", "#434c5e", [
    "#3b4252",
    "#bf616a",
    "#a3be8c",
    "#ebcb8b",
    "#81a1c1",
    "#b48ead",
    "#88c0d0",
    "#e5e9f0",
    "#4c566a",
    "#bf616a",
    "#a3be8c",
    "#ebcb8b",
    "#81a1c1",
    "#b48ead",
    "#8fbcbb",
    "#eceff4",
  ]),
  gruvbox_dark: pal("#282828", "#ebdbb2", "#ebdbb2", "#504945", [
    "#282828",
    "#cc241d",
    "#98971a",
    "#d79921",
    "#458588",
    "#b16286",
    "#689d6a",
    "#a89984",
    "#928374",
    "#fb4934",
    "#b8bb26",
    "#fabd2f",
    "#83a598",
    "#d3869b",
    "#8ec07c",
    "#ebdbb2",
  ]),
  one_dark: pal("#282c34", "#abb2bf", "#528bff", "#3e4451", [
    "#282c34",
    "#e06c75",
    "#98c379",
    "#e5c07b",
    "#61afef",
    "#c678dd",
    "#56b6c2",
    "#abb2bf",
    "#5c6370",
    "#e06c75",
    "#98c379",
    "#e5c07b",
    "#61afef",
    "#c678dd",
    "#56b6c2",
    "#ffffff",
  ]),
  tango_light: pal("#ffffff", "#2e3436", "#2e3436", "#c6d4e8", [
    "#2e3436",
    "#cc0000",
    "#4e9a06",
    "#c4a000",
    "#3465a4",
    "#75507b",
    "#06989a",
    "#d3d7cf",
    "#555753",
    "#ef2929",
    "#8ae234",
    "#fce94f",
    "#729fcf",
    "#ad7fa8",
    "#34e2e2",
    "#eeeeec",
  ]),
};

export function terminalTheme(theme: ResolvedTheme, scheme: TerminalScheme = "auto"): ITheme {
  // High contrast always wins: it is an accessibility setting.
  if (theme === "high_contrast") return highContrast;
  if (scheme !== "auto") return SCHEMES[scheme];
  return theme === "light" ? light : dark;
}
