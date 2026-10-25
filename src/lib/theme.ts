/**
 * Theme resolution (FR-053, ACC-002/004/005). Chrome colors live in CSS
 * custom properties keyed by `data-theme`; terminal palettes mirror them.
 */
import type { ITheme } from "@xterm/xterm";
import type { Settings, Theme } from "./ipc";

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

export function terminalTheme(theme: ResolvedTheme): ITheme {
  return theme === "light" ? light : theme === "high_contrast" ? highContrast : dark;
}
