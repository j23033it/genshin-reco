import "@testing-library/jest-dom/vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { TavilySettingsPanel } from "./TavilySettingsPanel";
import {
  deleteTavilyApiKey,
  readTavilySettingsStatus,
  saveTavilyApiKey,
  testTavilyConnection,
} from "./tavilySettings";

vi.mock("./tavilySettings", () => ({
  readTavilySettingsStatus: vi.fn(),
  saveTavilyApiKey: vi.fn(),
  testTavilyConnection: vi.fn(),
  deleteTavilyApiKey: vi.fn(),
}));

const readStatusMock = vi.mocked(readTavilySettingsStatus);
const saveMock = vi.mocked(saveTavilyApiKey);
const testMock = vi.mocked(testTavilyConnection);
const deleteMock = vi.mocked(deleteTavilyApiKey);

describe("TavilySettingsPanel", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    window.__TAURI_INTERNALS__ = {};
  });

  afterEach(() => {
    delete window.__TAURI_INTERNALS__;
  });

  it("APIキーの生値を取得せず設定状態だけを表示する", async () => {
    readStatusMock.mockResolvedValue({ configured: true });

    render(<TavilySettingsPanel />);

    expect(await screen.findByText("設定済み")).toBeInTheDocument();
    expect(screen.getByLabelText("Tavily APIキー")).toHaveValue("");
    expect(screen.getByRole("button", { name: "接続を確認" })).toBeEnabled();
  });

  it("入力したAPIキーを保存後に画面から消す", async () => {
    readStatusMock.mockResolvedValue({ configured: false });
    saveMock.mockResolvedValue({ configured: true });
    const user = userEvent.setup();
    render(<TavilySettingsPanel />);
    const input = await screen.findByLabelText("Tavily APIキー");

    await user.type(input, "tvly-secret");
    await user.click(screen.getByRole("button", { name: "APIキーを保存" }));

    expect(saveMock).toHaveBeenCalledWith("tvly-secret");
    expect(await screen.findByText("APIキーをOSの資格情報ストアへ保存しました。")).toBeInTheDocument();
    expect(input).toHaveValue("");
  });

  it("保存済みキーの接続確認と削除ができる", async () => {
    readStatusMock.mockResolvedValue({ configured: true });
    testMock.mockResolvedValue();
    deleteMock.mockResolvedValue({ configured: false });
    const user = userEvent.setup();
    render(<TavilySettingsPanel />);

    await user.click(await screen.findByRole("button", { name: "接続を確認" }));
    expect(await screen.findByText(/接続を確認できました/)).toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: "削除" }));
    expect(deleteMock).toHaveBeenCalledTimes(1);
    expect(await screen.findByText("未設定")).toBeInTheDocument();
  });
});
