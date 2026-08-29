import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import App from "./App";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));

import { invoke } from "@tauri-apps/api/core";

const mockedInvoke = vi.mocked(invoke);

describe("App shell", () => {
  beforeEach(() => {
    vi.resetAllMocks();
  });

  it("renders ToolOnize identity and framework shell badge", async () => {
    mockedInvoke.mockImplementation((cmd: string) => {
      if (cmd === "ping")
        return Promise.resolve({
          app_name: "ToolOnize",
          app_version: "0.1.0",
          target_os: "linux",
          target_arch: "x86_64",
          status: "ok",
        });
      if (cmd === "terminal_profiles") return Promise.resolve([]);
      if (cmd === "terminal_list") return Promise.resolve({ sessions: [] });
      return Promise.resolve(undefined);
    });
    render(<App />);
    expect(screen.getByRole("heading", { name: "ToolOnize" })).toBeInTheDocument();
    expect(
      screen.getByText("Your existing dev tools. One persistent workspace.")
    ).toBeInTheDocument();
    expect(screen.getByLabelText("Milestone")).toHaveTextContent("Terminal Core — M3");
    expect(await screen.findByText("0.1.0")).toBeInTheDocument();
  });

  it("shows loading state then renders sanitized ping data", async () => {
    mockedInvoke.mockImplementation((cmd: string) => {
      if (cmd === "ping")
        return Promise.resolve({
          app_name: "ToolOnize",
          app_version: "0.1.0",
          target_os: "linux",
          target_arch: "x86_64",
          status: "ok",
        });
      if (cmd === "terminal_profiles") return Promise.resolve([]);
      if (cmd === "terminal_list") return Promise.resolve({ sessions: [] });
      return Promise.resolve(undefined);
    });
    render(<App />);
    expect(screen.getByText("Contacting Rust core…")).toBeInTheDocument();
    await waitFor(() => expect(screen.getByText("ToolOnize")).toBeInTheDocument());
    // details
    expect(screen.getByText("Application")).toBeInTheDocument();
    expect(screen.getByText("Version")).toBeInTheDocument();
    expect(screen.getByText("Target OS")).toBeInTheDocument();
    expect(screen.getByText("Architecture")).toBeInTheDocument();
    expect(screen.getByText("0.1.0")).toBeInTheDocument();
    expect(screen.getByText("linux")).toBeInTheDocument();
    expect(screen.getByText("x86_64")).toBeInTheDocument();
  });

  it("does not crash on IPC failure and shows error affordance", async () => {
    mockedInvoke.mockImplementation((cmd: string) => {
      if (cmd === "ping") return Promise.reject(new Error("backend unreachable"));
      if (cmd === "terminal_profiles") return Promise.resolve([]);
      if (cmd === "terminal_list") return Promise.resolve({ sessions: [] });
      return Promise.resolve(undefined);
    });
    render(<App />);
    await waitFor(() => expect(screen.getByRole("alert")).toBeInTheDocument());
    expect(screen.getByText("backend unreachable")).toBeInTheDocument();
    expect(screen.getByText("IPC failed")).toBeInTheDocument();
    // badge still visible
    expect(screen.getByLabelText("Milestone")).toBeInTheDocument();
  });

  it("invokes only the ping command (and allowed terminal commands)", async () => {
    mockedInvoke.mockImplementation((cmd: string) => {
      if (cmd === "ping")
        return Promise.resolve({
          app_name: "ToolOnize",
          app_version: "0.1.0",
          target_os: "linux",
          target_arch: "x86_64",
          status: "ok",
        });
      if (cmd === "terminal_profiles") return Promise.resolve([]);
      if (cmd === "terminal_list") return Promise.resolve({ sessions: [] });
      return Promise.resolve(undefined);
    });
    render(<App />);
    await waitFor(() => expect(mockedInvoke).toHaveBeenCalled());
    expect(mockedInvoke).toHaveBeenCalledWith("ping");
    // ping must be called; terminal commands are allowed additional calls
    const calls = mockedInvoke.mock.calls.map((c) => c[0] as string);
    expect(calls).toContain("ping");
    for (const c of calls) {
      expect(["ping", "terminal_profiles", "terminal_list"]).toContain(c);
    }
  });
});
