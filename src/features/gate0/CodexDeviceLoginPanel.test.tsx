import "@testing-library/jest-dom/vitest";
import { act, fireEvent, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import {
  cancelCodexDeviceLogin,
  readCodexLoginStatus,
  startCodexDeviceLogin,
} from "./codexDeviceLogin";
import { CodexDeviceLoginPanel } from "./CodexDeviceLoginPanel";

vi.mock("./codexDeviceLogin", () => ({
  cancelCodexDeviceLogin: vi.fn(),
  readCodexLoginStatus: vi.fn(),
  startCodexDeviceLogin: vi.fn(),
}));

const startMock = vi.mocked(startCodexDeviceLogin);
const readMock = vi.mocked(readCodexLoginStatus);
const cancelMock = vi.mocked(cancelCodexDeviceLogin);
const challenge = {
  loginId: "login-123",
  verificationUrl: "https://auth.openai.com/codex/device",
  userCode: "ABCD-EFGH",
};
const pendingStatus = {
  authenticated: false,
  account: { authMode: null, planType: null, requiresOpenaiAuth: true },
  loginCompleted: null,
  loginError: null,
};

describe("CodexDeviceLoginPanel", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.useRealTimers();
    startMock.mockResolvedValue(challenge);
    readMock.mockResolvedValue(pendingStatus);
    cancelMock.mockResolvedValue(undefined);
  });

  afterEach(() => {
    vi.restoreAllMocks();
  });

  it("キーボード操作で認証を開始し、二重開始を受け付けない", async () => {
    let resolveStart: ((value: typeof challenge) => void) | undefined;
    startMock.mockReturnValue(
      new Promise((resolve) => {
        resolveStart = resolve;
      }),
    );
    const user = userEvent.setup();
    render(<CodexDeviceLoginPanel onRecheck={vi.fn()} />);

    const startButton = screen.getByRole("button", { name: "ログインを開始" });
    startButton.focus();
    await user.keyboard("{Enter}");
    expect(startMock).toHaveBeenCalledTimes(1);
    expect(screen.getByRole("status")).toHaveTextContent("準備しています");
    resolveStart?.(challenge);
    expect(await screen.findByText(challenge.userCode)).toBeInTheDocument();
  });

  it("URLとコードを表示し、コードのコピー成功を通知する", async () => {
    const writeText = vi.fn().mockResolvedValue(undefined);
    const user = userEvent.setup();
    Object.defineProperty(window.navigator, "clipboard", {
      configurable: true,
      value: { writeText },
    });
    render(<CodexDeviceLoginPanel onRecheck={vi.fn()} />);

    await user.click(screen.getByRole("button", { name: "ログインを開始" }));
    expect(await screen.findByText(challenge.verificationUrl)).toBeInTheDocument();
    expect(screen.getByText(challenge.userCode)).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "コードをコピー" }));

    expect(writeText).toHaveBeenCalledWith(challenge.userCode);
    expect(screen.getByRole("status")).toHaveTextContent("コピーしました");
  });

  it("クリップボードに失敗したときは手入力を案内する", async () => {
    const user = userEvent.setup();
    Object.defineProperty(window.navigator, "clipboard", {
      configurable: true,
      value: { writeText: vi.fn().mockRejectedValue(new Error("権限なし")) },
    });
    render(<CodexDeviceLoginPanel onRecheck={vi.fn()} />);

    await user.click(screen.getByRole("button", { name: "ログインを開始" }));
    await user.click(screen.getByRole("button", { name: "コードをコピー" }));

    expect(screen.getByRole("status")).toHaveTextContent("コピーできませんでした");
  });

  it("約2秒間隔で状態を確認し、成功後はポーリングを停止する", async () => {
    vi.useFakeTimers();
    readMock.mockResolvedValueOnce(pendingStatus).mockResolvedValueOnce({ ...pendingStatus, authenticated: true });
    const onRecheck = vi.fn();
    render(<CodexDeviceLoginPanel onRecheck={onRecheck} />);

    await act(async () => {
      fireEvent.click(screen.getByRole("button", { name: "ログインを開始" }));
    });
    expect(screen.getByText(challenge.userCode)).toBeInTheDocument();

    await act(async () => {
      await vi.advanceTimersByTimeAsync(2000);
    });
    expect(readMock).toHaveBeenCalledTimes(1);
    expect(screen.getByRole("status")).toHaveTextContent("状態を確認しています");

    await act(async () => {
      await vi.advanceTimersByTimeAsync(2000);
    });
    expect(readMock).toHaveBeenCalledTimes(2);
    expect(screen.getByRole("status")).toHaveTextContent("ログインが完了しました");
    expect(onRecheck).toHaveBeenCalledTimes(1);

    await act(async () => {
      await vi.advanceTimersByTimeAsync(4000);
    });
    expect(readMock).toHaveBeenCalledTimes(2);
  });

  it("完了通知だけでは成功扱いにせず、認証状態の反映を待つ", async () => {
    vi.useFakeTimers();
    readMock
      .mockResolvedValueOnce({ ...pendingStatus, loginCompleted: true })
      .mockResolvedValueOnce({ ...pendingStatus, authenticated: true });
    const onRecheck = vi.fn();
    render(<CodexDeviceLoginPanel onRecheck={onRecheck} />);

    await act(async () => {
      fireEvent.click(screen.getByRole("button", { name: "ログインを開始" }));
    });
    await act(async () => {
      await vi.advanceTimersByTimeAsync(2000);
    });
    expect(onRecheck).not.toHaveBeenCalled();
    expect(screen.getByText(challenge.userCode)).toBeInTheDocument();

    await act(async () => {
      await vi.advanceTimersByTimeAsync(2000);
    });
    expect(onRecheck).toHaveBeenCalledTimes(1);
  });

  it("認証を取消して開始状態へ戻る", async () => {
    const user = userEvent.setup();
    render(<CodexDeviceLoginPanel onRecheck={vi.fn()} />);

    await user.click(screen.getByRole("button", { name: "ログインを開始" }));
    await user.click(screen.getByRole("button", { name: "ログインを取消" }));

    expect(cancelMock).toHaveBeenCalledWith(challenge.loginId);
    expect(screen.getByRole("button", { name: "ログインを開始" })).toBeEnabled();
  });
});
