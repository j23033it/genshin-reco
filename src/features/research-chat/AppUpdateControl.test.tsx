import "@testing-library/jest-dom/vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { check, type Update } from "@tauri-apps/plugin-updater";
import { relaunch } from "@tauri-apps/plugin-process";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { AppUpdateControl } from "./AppUpdateControl";

vi.mock("@tauri-apps/plugin-updater", () => ({ check: vi.fn() }));
vi.mock("@tauri-apps/plugin-process", () => ({ relaunch: vi.fn() }));

const checkMock = vi.mocked(check);
const relaunchMock = vi.mocked(relaunch);

describe("デスクトップアプリの更新", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.stubEnv("DEV", false);
    window.__TAURI_INTERNALS__ = {};
  });

  afterEach(() => {
    vi.unstubAllEnvs();
    delete window.__TAURI_INTERNALS__;
  });

  it("最新版なら更新しない", async () => {
    checkMock.mockResolvedValue(null);
    const user = userEvent.setup();
    render(<AppUpdateControl />);

    await user.click(screen.getByRole("button", { name: "更新を確認" }));
    expect(await screen.findByRole("status")).toHaveTextContent("最新版です");
    expect(relaunchMock).not.toHaveBeenCalled();
  });

  it("新しい版は明示操作の後だけ導入して再起動する", async () => {
    const downloadAndInstall = vi.fn().mockResolvedValue(undefined);
    checkMock.mockResolvedValue({
      version: "0.2.0",
      downloadAndInstall,
    } as unknown as Update);
    const user = userEvent.setup();
    render(<AppUpdateControl />);

    await user.click(screen.getByRole("button", { name: "更新を確認" }));
    expect(await screen.findByText("新しい版 0.2.0")).toBeInTheDocument();
    expect(downloadAndInstall).not.toHaveBeenCalled();

    await user.click(screen.getByRole("button", { name: "更新して再起動" }));
    expect(downloadAndInstall).toHaveBeenCalledTimes(1);
    expect(relaunchMock).toHaveBeenCalledTimes(1);
  });

  it("取得失敗を表示して再試行できる", async () => {
    checkMock.mockRejectedValueOnce(new Error("接続失敗")).mockResolvedValueOnce(null);
    const user = userEvent.setup();
    render(<AppUpdateControl />);

    await user.click(screen.getByRole("button", { name: "更新を確認" }));
    expect(await screen.findByRole("alert")).toHaveTextContent("接続失敗");
    await user.click(screen.getByRole("button", { name: "更新を確認" }));
    expect(await screen.findByText("最新版です")).toBeInTheDocument();
  });

  it("ブラウザのデモと開発版では更新を出さない", () => {
    delete window.__TAURI_INTERNALS__;
    const { rerender } = render(<AppUpdateControl />);
    expect(screen.queryByRole("button", { name: "更新を確認" })).not.toBeInTheDocument();

    window.__TAURI_INTERNALS__ = {};
    vi.stubEnv("DEV", true);
    rerender(<AppUpdateControl />);
    expect(screen.queryByRole("button", { name: "更新を確認" })).not.toBeInTheDocument();
  });
});
