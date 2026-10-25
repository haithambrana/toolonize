import "@testing-library/jest-dom/vitest";
import { vi } from "vitest";

// Every test talks to the in-memory fake core instead of Tauri.
vi.mock("@tauri-apps/api/core", async () => {
  const fake = await import("./fakeCore");
  return { invoke: fake.invoke, Channel: fake.FakeChannel };
});
vi.mock("@xterm/xterm", async () => {
  const fake = await import("./fakeXterm");
  return { Terminal: fake.FakeTerminal };
});
vi.mock("@xterm/addon-fit", async () => {
  const fake = await import("./fakeXterm");
  return { FitAddon: fake.FakeFitAddon };
});
vi.mock("@xterm/addon-search", async () => {
  const fake = await import("./fakeXterm");
  return { SearchAddon: fake.FakeSearchAddon };
});

// jsdom has no layout: give elements a fixed size and report it once to
// ResizeObserver subscribers so FlexLayout renders its tabs.
class FakeResizeObserver {
  constructor(private callback: ResizeObserverCallback) {}
  observe(target: Element) {
    setTimeout(
      () => this.callback([{ target } as ResizeObserverEntry], this as unknown as ResizeObserver),
      0
    );
  }
  unobserve() {}
  disconnect() {}
}
(globalThis as unknown as { ResizeObserver: typeof FakeResizeObserver }).ResizeObserver =
  FakeResizeObserver;
Element.prototype.getBoundingClientRect = function () {
  return {
    x: 0,
    y: 0,
    top: 0,
    left: 0,
    right: 1000,
    bottom: 700,
    width: 1000,
    height: 700,
    toJSON() {},
  };
};
if (typeof window.matchMedia !== "function") {
  window.matchMedia = ((query: string) => ({
    matches: false,
    media: query,
    onchange: null,
    addListener: () => undefined,
    removeListener: () => undefined,
    addEventListener: () => undefined,
    removeEventListener: () => undefined,
    dispatchEvent: () => false,
  })) as unknown as typeof window.matchMedia;
}
