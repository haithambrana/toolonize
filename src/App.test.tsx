import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import App from "./App";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));

import { invoke } from "@tauri-apps/api/core";

const mockedInvoke = vi.mocked(invoke);

const emptyList = () => ({ sessions: [] });

const okInvoke = (cmd: string) => {
  if (cmd === "terminal_profiles") return Promise.resolve([]);
  if (cmd === "terminal_list") return Promise.resolve(emptyList());
  return Promise.resolve(undefined);
};

describe("M4 Workspace shell", () => {
  beforeEach(() => {
    vi.resetAllMocks();
  });

  it("renders ToolOnize identity and profile selector with no sessions", async () => {
    mockedInvoke.mockImplementation(okInvoke);
    render(<App />);
    expect(await screen.findByText("ToolOnize")).toBeInTheDocument();
    expect(screen.getByText("Workspace — M4")).toBeInTheDocument();
    expect(screen.getByLabelText("Shell profile")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Start terminal" })).toBeInTheDocument();
    await waitFor(() =>
      expect(screen.getByText("No sessions yet. Start one above.")).toBeInTheDocument()
    );
  });

  it("invokes only allowed terminal commands (profiles, list)", async () => {
    mockedInvoke.mockImplementation(okInvoke);
    render(<App />);
    await screen.findByText("ToolOnize");
    await waitFor(() => expect(mockedInvoke).toHaveBeenCalled());
    const calls = mockedInvoke.mock.calls.map((c) => c[0] as string);
    expect(calls).toContain("terminal_profiles");
    expect(calls).toContain("terminal_list");
    for (const c of calls) {
      expect(["terminal_profiles", "terminal_list"]).toContain(c);
    }
  });

  it("surfaces profile load failure as an alert", async () => {
    mockedInvoke.mockImplementation((cmd: string) => {
      if (cmd === "terminal_profiles") return Promise.reject(new Error("profiles unreachable"));
      if (cmd === "terminal_list") return Promise.resolve(emptyList());
      return Promise.resolve(undefined);
    });
    render(<App />);
    await waitFor(() => expect(screen.getByRole("alert")).toBeInTheDocument());
    expect(screen.getByText(/profiles unreachable/)).toBeInTheDocument();
  });

  it("places session controls in the status bar without a selected session", async () => {
    mockedInvoke.mockImplementation(okInvoke);
    render(<App />);
    await waitFor(() => expect(screen.getByText("No session selected")).toBeInTheDocument());
  });
});
