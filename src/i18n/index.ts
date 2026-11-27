/**
 * Translations without a dependency. The English text is the key, so a
 * string without a translation still shows (in English) instead of an id.
 * Arabic switches the page to right-to-left; terminals stay left-to-right.
 */
import { useSyncExternalStore } from "react";
import { ar } from "./ar";

export type Language = "en" | "ar";
export type LanguagePref = "system" | Language;

let current: Language = "en";
const listeners = new Set<() => void>();

export function resolveLanguage(pref: LanguagePref, system = navigator.language ?? "en"): Language {
  if (pref === "system") return system.toLowerCase().startsWith("ar") ? "ar" : "en";
  return pref;
}

export function setLanguage(pref: LanguagePref): void {
  const lang = resolveLanguage(pref);
  document.documentElement.lang = lang;
  document.documentElement.dir = lang === "ar" ? "rtl" : "ltr";
  if (lang === current) return;
  current = lang;
  for (const l of listeners) l();
}

export function language(): Language {
  return current;
}

/** Marks text in module-level constants for translation; translate it with
 * t() where it is shown (the language is not known when modules load). */
export function tk(key: string): string {
  return key;
}

/** Translate `key`; `{name}` placeholders are filled from `vars`. */
export function t(key: string, vars?: Record<string, string | number>): string {
  const text = current === "ar" ? (ar[key] ?? key) : key;
  if (!vars) return text;
  return text.replace(/\{(\w+)\}/g, (m, k: string) => (k in vars ? String(vars[k]) : m));
}

export function useLanguage(): Language {
  return useSyncExternalStore(
    (l) => {
      listeners.add(l);
      return () => listeners.delete(l);
    },
    () => current
  );
}
