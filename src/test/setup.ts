import "@testing-library/jest-dom/vitest";

// Polyfill ResizeObserver for jsdom (xterm FitAddon uses it via TerminalView)
if (typeof globalThis.ResizeObserver === "undefined") {
  globalThis.ResizeObserver = class {
    observe() {}
    unobserve() {}
    disconnect() {}
    // eslint-disable-next-line @typescript-eslint/no-unused-vars
    constructor(_cb: ResizeObserverCallback) {}
  } as unknown as typeof ResizeObserver;
}
