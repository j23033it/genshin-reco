import "@testing-library/jest-dom/vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { readCodexLoginStatus } from "../gate0/codexDeviceLogin";
import { CodexConnectionGate } from "./CodexConnectionGate";

vi.mock("../gate0/codexDeviceLogin", () => ({
  readCodexLoginStatus: vi.fn(),
}));
vi.mock("../gate0/CodexDeviceLoginPanel", () => ({
  CodexDeviceLoginPanel: ({ onRecheck }: { onRecheck: () => void }) => (
    <button type="button" onClick={onRecheck}>ログイン完了を確認</button>
  ),
}));
vi.mock("./ResearchChatPrototype", () => ({
  default: () => <h1>編成調査画面</h1>,
}));

const readMock = vi.mocked(readCodexLoginStatus);
const disconnected = {
  authenticated: false,
  account: { authMode: null, planType: null, requiresOpenaiAuth: true },
  loginCompleted: null,
  loginError: null,
};

describe("Codex接続から編成調査への導線", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    window.__TAURI_INTERNALS__ = {};
  });

  afterEach(() => {
    delete window.__TAURI_INTERNALS__;
  });

  it("未認証ならログインを示し、認証後に調査画面へ進む", async () => {
    readMock.mockResolvedValueOnce(disconnected).mockResolvedValueOnce({
      ...disconnected,
      authenticated: true,
      account: { authMode: "chatgpt", planType: "plus", requiresOpenaiAuth: true },
    });
    const user = userEvent.setup();
    render(<CodexConnectionGate />);

    await user.click(await screen.findByRole("button", { name: "ログイン完了を確認" }));
    expect(await screen.findByRole("heading", { name: "編成調査画面" })).toBeInTheDocument();
    expect(readMock).toHaveBeenCalledTimes(2);
  });

  it("ブラウザだけで開いた場合はデスクトップ起動を案内する", () => {
    delete window.__TAURI_INTERNALS__;
    render(<CodexConnectionGate />);

    expect(screen.getByText(/npm run tauri dev/)).toBeInTheDocument();
    expect(readMock).not.toHaveBeenCalled();
  });
});
