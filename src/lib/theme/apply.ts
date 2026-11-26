import { THEMES } from "./themes";
import type { ThemeMode } from "../settings.svelte";

export function systemPrefersDark(): boolean {
  return window.matchMedia("(prefers-color-scheme: dark)").matches;
}

export function isDark(mode: ThemeMode): boolean {
  return mode === "dark" || (mode === "system" && systemPrefersDark());
}

/** Writes the theme's Material 3 roles as CSS variables (`--primary`, `--on-surface`, ...). */
export function applyTheme(themeId: string, mode: ThemeMode, pureBlack: boolean) {
  const theme = THEMES.find((t) => t.id === themeId) ?? THEMES[0];
  const dark = isDark(mode);
  const scheme = { ...(dark ? theme.dark : theme.light) };
  if (dark && pureBlack) {
    // Same idea as Mihon's AMOLED mode: black backgrounds, containers stay distinguishable.
    for (const k of ["background", "surface", "surface-container-lowest", "surface-container-low"]) scheme[k] = "#000000";
    scheme["surface-container"] = "#0c0c0c";
    scheme["surface-variant"] = "#0c0c0c";
  }
  const root = document.documentElement;
  for (const [k, v] of Object.entries(scheme)) root.style.setProperty(`--${k}`, v);
  root.dataset.dark = String(dark);
  root.style.colorScheme = dark ? "dark" : "light";
}
