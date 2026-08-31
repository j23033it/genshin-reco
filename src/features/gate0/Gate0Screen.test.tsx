import "@testing-library/jest-dom/vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { Gate0Screen } from "./Gate0Screen";
import { probeCodexEnvironment } from "./probeCodexEnvironment";
import type { Gate0ProbeReport } from "./types";

vi.mock("./probeCodexEnvironment", () => ({
  probeCodexEnvironment: vi.fn(),
}));

const probeMock = vi.mocked(probeCodexEnvironment);

const report = (connected: boolean, rateLimitsAvailable = true): Gate0ProbeReport => ({
  codexPath: "C:\\codex.exe",
  codexVersion: "0.118.0",
  versionSupported: true,
  appServerInitialized: true,
  isolatedHome: "C:\\codex-home",
  platformFamily: "windows",
  platformOs: "windows",
  account: {
    authMode: connected ? "chatgpt" : null,
    planType: connected ? "prolite" : null,
    requiresOpenaiAuth: true,
  },
  rateLimitsAvailable,
  diagnostics: [],
});

describe("Gate0Screen", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    window.__TAURI_INTERNALS__ = {};
  });

  afterEach(() => {
    delete window.__TAURI_INTERNALS__;
  });

  it("画面を開くと自動確認し、ChatGPT認証済みなら連携済みと表示する", async () => {
    probeMock.mockResolvedValue(report(true));

    render(<Gate0Screen embedded />);

    await screen.findByText("連携済み");
    const status = screen.getByTestId("codex-connection-status");
    expect(status).toHaveTextContent("連携済み");
    expect(status).toHaveTextContent("このアプリはCodexと連携済みです");
    expect(screen.queryByRole("button", { name: "ログインを開始" })).not.toBeInTheDocument();
    expect(probeMock).toHaveBeenCalledTimes(1);
  });

  it("利用上限を取得できなくても連携済み表示を維持する", async () => {
    probeMock.mockResolvedValue(report(true, false));

    render(<Gate0Screen embedded />);

    expect(await screen.findByText("連携済み")).toBeInTheDocument();
    expect(screen.getByText("取得できません（連携済み）")).toBeInTheDocument();
    expect(screen.getByText(/利用上限情報だけを現在表示できません/)).toBeInTheDocument();
    expect(screen.queryByText("Codexへログインしてください")).not.toBeInTheDocument();
  });

  it("認証方式がなければ未連携とログイン操作を表示する", async () => {
    probeMock.mockResolvedValue(report(false, false));

    render(<Gate0Screen embedded />);

    expect(await screen.findByText("未連携")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "ログインを開始" })).toBeEnabled();
  });

  it("自動確認に失敗しても再確認できる", async () => {
    probeMock
      .mockRejectedValueOnce(new Error("Codexへ接続できませんでした。"))
      .mockResolvedValueOnce(report(true));
    const user = userEvent.setup();

    render(<Gate0Screen embedded />);

    expect(await screen.findByText("確認できません")).toBeInTheDocument();
    expect(screen.getByRole("alert")).toHaveTextContent("Codexへ接続できませんでした");
    await user.click(screen.getByRole("button", { name: "再確認" }));

    expect(await screen.findByText("連携済み")).toBeInTheDocument();
    expect(probeMock).toHaveBeenCalledTimes(2);
  });
});
