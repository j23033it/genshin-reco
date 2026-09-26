import "@testing-library/jest-dom/vitest";
import { act, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { check, type DownloadEvent, type Update } from "@tauri-apps/plugin-updater";
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
    relaunchMock.mockResolvedValue(undefined);
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

  it("受信量に合わせてバーを進め、取得後は適用と再起動を待つ表示へ切り替える", async () => {
    let emit!: (event: DownloadEvent) => void;
    let finish!: () => void;
    const downloadAndInstall = vi.fn((callback: (event: DownloadEvent) => void) => {
      emit = callback;
      return new Promise<void>((resolve) => { finish = resolve; });
    });
    checkMock.mockResolvedValue({ version: "0.2.0", downloadAndInstall } as unknown as Update);
    relaunchMock.mockReturnValue(new Promise(() => {}));
    const user = userEvent.setup();
    render(<AppUpdateControl />);
    await user.click(screen.getByRole("button", { name: "更新を確認" }));
    await user.click(screen.getByRole("button", { name: "更新して再起動" }));

    expect(screen.getByRole("progressbar")).not.toHaveAttribute("aria-valuenow");
    act(() => emit({ event: "Started", data: { contentLength: 1000 } }));
    expect(screen.getByRole("progressbar")).toHaveAttribute("aria-valuenow", "0");
    act(() => emit({ event: "Progress", data: { chunkLength: 250 } }));
    expect(screen.getByRole("progressbar")).toHaveAttribute("aria-valuenow", "25");
    act(() => emit({ event: "Progress", data: { chunkLength: 300 } }));
    expect(Number(screen.getByRole("progressbar").getAttribute("aria-valuenow"))).toBeCloseTo(55);
    expect(screen.getByText("55%")).toBeInTheDocument();
    act(() => emit({ event: "Progress", data: { chunkLength: 500 } }));
    expect(screen.getByRole("progressbar")).toHaveAttribute("aria-valuenow", "100");
    act(() => emit({ event: "Finished" }));
    expect(screen.getByRole("progressbar", { name: "更新を適用中…" })).not.toHaveAttribute("aria-valuenow");
    await act(async () => finish());
    expect(screen.getByRole("progressbar", { name: "再起動中…" })).not.toHaveAttribute("aria-valuenow");
    expect(relaunchMock).toHaveBeenCalledTimes(1);
  });

  it.each([undefined, 0])("総量が%sなら円を表示し、失敗後は消して再試行できる", async (contentLength) => {
    let emit!: (event: DownloadEvent) => void;
    let fail!: (error: Error) => void;
    const downloadAndInstall = vi.fn((callback: (event: DownloadEvent) => void) => {
      emit = callback;
      return new Promise<void>((_resolve, reject) => { fail = reject; });
    });
    checkMock.mockResolvedValue({ version: "0.2.0", downloadAndInstall } as unknown as Update);
    const user = userEvent.setup();
    render(<AppUpdateControl />);
    await user.click(screen.getByRole("button", { name: "更新を確認" }));
    await user.click(screen.getByRole("button", { name: "更新して再起動" }));
    act(() => {
      emit({ event: "Started", data: { contentLength } });
      emit({ event: "Progress", data: { chunkLength: 250 } });
    });
    expect(screen.getByRole("progressbar")).not.toHaveAttribute("aria-valuenow");
    expect(screen.queryByText(/\d+%/)).not.toBeInTheDocument();
    await act(async () => fail(new Error("取得失敗")));
    expect(screen.getByRole("alert")).toHaveTextContent("取得失敗");
    expect(screen.queryByRole("progressbar")).not.toBeInTheDocument();
    expect(screen.getByRole("button", { name: "更新を確認" })).toBeEnabled();
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
