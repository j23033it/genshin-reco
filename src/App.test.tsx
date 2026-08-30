import "@testing-library/jest-dom/vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import App from "./App";
import { probeCodexEnvironment } from "./features/gate0/probeCodexEnvironment";
import type { Gate0ProbeReport } from "./features/gate0/types";

vi.mock("./features/gate0/probeCodexEnvironment", () => ({
  probeCodexEnvironment: vi.fn(),
}));

const probeMock = vi.mocked(probeCodexEnvironment);
const successfulReport = {
  codexPath: "C:\\Tools\\codex.exe",
  codexVersion: "1.2.3",
  versionSupported: true,
  appServerInitialized: true,
  isolatedHome: "C:\\Temp\\codex-home",
  platformFamily: "windows",
  platformOs: "Windows 11",
  account: {
    authMode: "chatgpt",
    planType: "pro",
    requiresOpenaiAuth: false,
  },
  rateLimitsAvailable: true,
  diagnostics: ["接続確認済み"],
};

describe("Gate 0", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("初期状態では環境を確認する操作を表示する", () => {
    render(<App />);

    expect(screen.getByRole("heading", { name: "根拠付き・編成連動ビルド推薦" })).toBeInTheDocument();
    expect(screen.getByRole("heading", { name: "Codexの環境を確認" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "環境を確認" })).toBeEnabled();
    expect(screen.getByRole("status")).toHaveTextContent("Gate 0");
  });

  it("確認中はボタンを無効化し、二重実行を防ぐ", async () => {
    let resolveProbe: ((value: Gate0ProbeReport) => void) | undefined;
    probeMock.mockReturnValue(
      new Promise<Gate0ProbeReport>((resolve) => {
        resolveProbe = resolve;
      }),
    );
    const user = userEvent.setup();
    render(<App />);

    const button = screen.getByRole("button", { name: "環境を確認" });
    await user.click(button);
    expect(screen.getByRole("button", { name: "確認中…" })).toBeDisabled();
    expect(screen.getByTestId("gate0-loading")).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "確認中…" }));
    expect(probeMock).toHaveBeenCalledTimes(1);

    resolveProbe?.(successfulReport);
  });

  it("成功時は確認結果を項目別に表示する", async () => {
    probeMock.mockResolvedValue(successfulReport);
    const user = userEvent.setup();
    render(<App />);

    await user.click(screen.getByRole("button", { name: "環境を確認" }));

    expect(await screen.findByTestId("gate0-report")).toBeInTheDocument();
    expect(screen.getByText("C:\\Tools\\codex.exe")).toBeInTheDocument();
    expect(screen.getByText("1.2.3")).toBeInTheDocument();
    expect(screen.getByText("Windows 11")).toBeInTheDocument();
    expect(screen.getByText("接続確認済み")).toBeInTheDocument();
    expect(screen.getByRole("status")).toHaveTextContent("完了");
  });

  it("失敗時は操作の近くにエラーを表示し、再実行できる", async () => {
    probeMock.mockRejectedValue(new Error("App Serverに接続できません"));
    const user = userEvent.setup();
    render(<App />);

    await user.click(screen.getByRole("button", { name: "環境を確認" }));

    expect(await screen.findByRole("alert")).toHaveTextContent("App Serverに接続できません");
    expect(screen.getByRole("button", { name: "環境を確認" })).toBeEnabled();
  });

  it("未認証の場合はログイン案内を表示する", async () => {
    probeMock.mockResolvedValue({ ...successfulReport, account: null });
    const user = userEvent.setup();
    render(<App />);

    await user.click(screen.getByRole("button", { name: "環境を確認" }));

    expect(await screen.findByText("Codexへログインしてください")).toBeInTheDocument();
  });

  it("ChatGPT以外の認証方式では実ターンスモークを表示しない", async () => {
    probeMock.mockResolvedValue({
      ...successfulReport,
      account: { authMode: "apiKey", planType: null, requiresOpenaiAuth: false },
    });
    const user = userEvent.setup();
    render(<App />);

    await user.click(screen.getByRole("button", { name: "環境を確認" }));

    expect(await screen.findByText(/実ターンのスモークテストにはChatGPT認証が必要/)).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Gate 0スモークを実行" })).not.toBeInTheDocument();
  });
});
