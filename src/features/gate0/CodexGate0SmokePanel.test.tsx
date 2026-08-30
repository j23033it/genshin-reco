import "@testing-library/jest-dom/vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { runCodexGate0Smoke } from "./codexGate0Smoke";
import { CodexGate0SmokePanel } from "./CodexGate0SmokePanel";

vi.mock("./codexGate0Smoke", () => ({
  runCodexGate0Smoke: vi.fn(),
}));

const runMock = vi.mocked(runCodexGate0Smoke);
const passingReport = {
  structuredOutputValid: true,
  webSearchObserved: true,
  cancellationObserved: true,
  instructionSourcesSupported: false,
  modelRerouted: true,
  reroutedFrom: "gpt-5.4",
  reroutedTo: "gpt-5.4-mini",
};

describe("CodexGate0SmokePanel", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    runMock.mockResolvedValue(passingReport);
  });

  it("実ターンスモークの合格項目と互換情報を表示する", async () => {
    const user = userEvent.setup();
    render(<CodexGate0SmokePanel />);

    await user.click(screen.getByRole("button", { name: "Gate 0スモークを実行" }));

    expect(await screen.findByText("Gate 0スモークは合格です。")).toBeInTheDocument();
    expect(screen.getAllByText("合格")).toHaveLength(3);
    expect(screen.getByText(/Codex 0\.118互換モード/)).toBeInTheDocument();
    expect(screen.getByText(/gpt-5\.4 → gpt-5\.4-mini/)).toBeInTheDocument();
  });

  it("実行中はボタンを無効化して二重送信を防ぐ", async () => {
    let resolveRun: ((value: typeof passingReport) => void) | undefined;
    runMock.mockReturnValue(
      new Promise((resolve) => {
        resolveRun = resolve;
      }),
    );
    const user = userEvent.setup();
    render(<CodexGate0SmokePanel />);

    const runButton = screen.getByRole("button", { name: "Gate 0スモークを実行" });
    await user.click(runButton);

    expect(screen.getByRole("button", { name: "スモーク実行中…" })).toBeDisabled();
    expect(runMock).toHaveBeenCalledTimes(1);
    resolveRun?.(passingReport);
    expect(await screen.findByText("Gate 0スモークは合格です。")).toBeInTheDocument();
  });

  it("Tauriコマンドの失敗を操作可能なエラーとして表示する", async () => {
    runMock.mockRejectedValue(new Error("Codex認証が必要です。"));
    const user = userEvent.setup();
    render(<CodexGate0SmokePanel />);

    await user.click(screen.getByRole("button", { name: "Gate 0スモークを実行" }));

    expect(await screen.findByRole("alert")).toHaveTextContent("Codex認証が必要です。");
    expect(screen.getByRole("button", { name: "Gate 0スモークを実行" })).toBeEnabled();
  });
});
