import { readdirSync, readFileSync, statSync } from "node:fs";
import { join } from "node:path";
import { afterEach, describe, expect, it } from "vitest";
import { ar } from "./ar";
import { language, resolveLanguage, setLanguage, t } from "./index";

/** Every literal key passed to t() in the app's own sources. */
function usedKeys(): Set<string> {
  const keys = new Set<string>();
  const walk = (dir: string) => {
    for (const name of readdirSync(dir)) {
      const path = join(dir, name);
      if (statSync(path).isDirectory()) {
        if (!["test", "spike", "i18n"].includes(name)) walk(path);
      } else if (/\.tsx?$/.test(name) && !/\.test\./.test(name)) {
        const src = readFileSync(path, "utf8");
        for (const m of src.matchAll(/(?<![\w.])tk?\(\s*"((?:[^"\\]|\\.)*)"/g)) {
          keys.add(JSON.parse(`"${m[1]}"`) as string);
        }
      }
    }
  };
  walk(join(__dirname, ".."));
  return keys;
}

const placeholders = (s: string) => [...s.matchAll(/\{(\w+)\}/g)].map((m) => m[1]).sort();

describe("translations", () => {
  afterEach(() => setLanguage("en"));

  it("every text shown through t() has an Arabic translation", () => {
    const missing = [...usedKeys()].filter((k) => !(k in ar));
    expect(missing).toEqual([]);
  });

  it("no stale Arabic entries and placeholders match", () => {
    const used = usedKeys();
    const stale = Object.keys(ar).filter((k) => !used.has(k));
    expect(stale).toEqual([]);
    for (const [en, tr] of Object.entries(ar)) {
      expect(placeholders(tr), en).toEqual(placeholders(en));
      expect(tr.trim(), en).not.toBe("");
    }
    expect(used.size).toBeGreaterThan(300);
  });

  it("fills placeholders and falls back to English", () => {
    expect(t("Delete “{name}”?", { name: "Ops" })).toBe("Delete “Ops”?");
    setLanguage("ar");
    expect(t("Delete “{name}”?", { name: "Ops" })).toBe("حذف «Ops»؟");
    expect(t("A text nobody translated")).toBe("A text nobody translated");
    expect(t("{n} terminal(s)", {})).toBe("{n} تيرمنال");
  });

  it("Arabic turns the page right-to-left; English and system follow suit", () => {
    setLanguage("ar");
    expect(language()).toBe("ar");
    expect(document.documentElement.dir).toBe("rtl");
    expect(document.documentElement.lang).toBe("ar");
    setLanguage("en");
    expect(document.documentElement.dir).toBe("ltr");
    expect(resolveLanguage("system", "ar-EG")).toBe("ar");
    expect(resolveLanguage("system", "en-US")).toBe("en");
    expect(resolveLanguage("en", "ar-EG")).toBe("en");
  });
});
