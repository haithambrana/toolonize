import { describe, it, expect } from "vitest";
import { readFileSync } from "node:fs";
import { join } from "node:path";

const appCss = readFileSync(join(process.cwd(), "src/styles/app.css"), "utf8");
const termCss = readFileSync(join(process.cwd(), "src/terminal/terminal.css"), "utf8");
const appTsx = readFileSync(join(process.cwd(), "src/App.tsx"), "utf8");
const coreTsx = readFileSync(join(process.cwd(), "src/terminal/TerminalCore.tsx"), "utf8");

describe("M3 desktop usability correction — layout regression", () => {
  it("APP_USES_DESKTOP_WIDTH: shell uses width 100% and max-width 1500-1700", () => {
    expect(appCss).toMatch(/\.shell\s*\{[^}]*width:\s*100%/s);
    const m = appCss.match(/\.shell\s*\{[^}]*max-width:\s*(\d+)px/s);
    expect(m).not.toBeNull();
    const w = m ? parseInt(m[1], 10) : 0;
    expect(w).toBeGreaterThanOrEqual(1500);
    expect(w).toBeLessThanOrEqual(1700);
  });

  it("TERMINAL_CORE_FULL_WIDTH: dedicated class with grid-column 1 / -1", () => {
    expect(appCss).toMatch(/\.terminal-core--full[^{]*\{[^}]*grid-column:\s*1\s*\/\s*-1/s);
    expect(appTsx).toContain("terminal-core--full");
    expect(appTsx).toContain("terminal-core-section");
  });

  it("SIDEBAR_WIDTH: 240-300px proportion", () => {
    expect(termCss).toMatch(/grid-template-columns:\s*minmax\(240px,\s*300px\)/);
  });

  it("TERMINAL_MIN_WIDTH_ZERO: terminal region can shrink", () => {
    expect(termCss).toMatch(/\.terminal-core__main[^{]*\{[^}]*min-width:\s*0/s);
    expect(termCss).toMatch(/\.terminal-view[^{]*\{[^}]*min-width:\s*0/s);
    expect(appCss).toMatch(/\.terminal-core-section[^}]*min-width:\s*0/s);
  });

  it("TERMINAL_DESKTOP_HEIGHT: viewport at least ~600px", () => {
    expect(termCss).toMatch(/\.terminal-core__main[^{]*\{[^}]*min-height:\s*600px/s);
    expect(termCss).toMatch(/\.terminal-core__main\s+\.terminal-view[^{]*\{[^}]*min-height:\s*600px/s);
  });

  it("RESPONSIVE_STACK: narrow viewport stacks via max-width 850-950", () => {
    expect(termCss).toMatch(/@media\s*\(max-width:\s*900px\)/);
    const block = termCss.match(/@media\s*\(max-width:\s*900px\)[\s\S]*?\{[\s\S]*?\.terminal-core__layout[\s\S]*?grid-template-columns:\s*1fr/s);
    expect(block).not.toBeNull();
  });

  it("DEBUG_INFO_DEMOTED: diagnostics demoted via collapsible details", () => {
    expect(coreTsx).toContain('<details className="terminal-core__diagnostics"');
    expect(coreTsx).toContain('<details className="terminal-core__session-details"');
    expect(appTsx).toContain('<details className="diagnostics"');
    expect(termCss).toContain(".terminal-core__diagnostics");
    expect(appCss).toContain(".diagnostics");
  });

  it("INLINE_LAYOUT_REDUCED: no fixed 340px inline grid in TerminalCore", () => {
    expect(coreTsx).not.toContain("340px");
    expect(coreTsx).not.toContain("gridTemplateColumns");
    expect(coreTsx).not.toContain('style={{ display: "grid"');
    expect(coreTsx).not.toContain("minHeight: 380");
    expect(coreTsx).not.toContain("minHeight: 400");
  });

  it("no horizontal overflow: long values truncate/wrap safely", () => {
    expect(appCss).toMatch(/\.detail-row\s+dd[^}]*overflow-wrap:\s*anywhere/s);
    expect(termCss).toMatch(/\.session-row__id[^}]*text-overflow:\s*ellipsis/s);
  });
});
