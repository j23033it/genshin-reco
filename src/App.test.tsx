import "@testing-library/jest-dom/vitest";
import { act, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import App from "./App";
import type { Catalog } from "./domain/catalogTypes";
import { loadCurrentAnalysisResult } from "./features/analysis";
import { loadCatalog } from "./features/catalog";
import { probeCodexEnvironment } from "./features/gate0/probeCodexEnvironment";
import { listPartyDrafts, loadPartyDraft, savePartyDraft } from "./features/party";
import type { PartyDraft } from "./features/party";

vi.mock("./features/catalog", () => ({
  loadCatalog: vi.fn(),
}));

vi.mock("./features/gate0/probeCodexEnvironment", () => ({
  probeCodexEnvironment: vi.fn(),
}));

vi.mock("./features/analysis", async (importOriginal) => ({
  ...(await importOriginal<typeof import("./features/analysis")>()),
  loadCurrentAnalysisResult: vi.fn(),
}));

vi.mock("./features/party", async (importOriginal) => ({
  ...(await importOriginal<typeof import("./features/party")>()),
  listPartyDrafts: vi.fn(),
  loadPartyDraft: vi.fn(),
  savePartyDraft: vi.fn(),
}));

const loadCatalogMock = vi.mocked(loadCatalog);
const probeCodexEnvironmentMock = vi.mocked(probeCodexEnvironment);
const listPartyDraftsMock = vi.mocked(listPartyDrafts);
const loadPartyDraftMock = vi.mocked(loadPartyDraft);
const savePartyDraftMock = vi.mocked(savePartyDraft);
const loadCurrentAnalysisResultMock = vi.mocked(loadCurrentAnalysisResult);
const savedParty: PartyDraft = {
  partyId: "saved-party",
  id: "saved-party",
  name: "保存済み編成",
  members: [0, 1, 2, 3].map((slotIndex) => ({
    slotIndex: slotIndex as 0 | 1 | 2 | 3,
    characterId: null,
    weaponId: null,
    constellation: 0,
    refinement: 1,
  })) as PartyDraft["members"],
};
const catalog: Catalog = {
  schemaVersion: "catalog-v2",
  gameVersion: "7.0",
  catalogUpdatedAt: "2026-08-30",
  characters: [
    {
      id: "char-a",
      name: "キャラA",
      element: "炎",
      weaponType: "片手剣",
      rarity: 5,
      imageUrl: "https://example.com/a.png",
    },
  ],
  weapons: [
    {
      id: "weapon-a",
      name: "武器A",
      weaponType: "片手剣",
      rarity: 5,
      imageUrl: "https://example.com/w.png",
    },
  ],
  artifactSets: [],
};

describe("アプリワークスペース", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    loadCatalogMock.mockResolvedValue(catalog);
    listPartyDraftsMock.mockResolvedValue([]);
    loadPartyDraftMock.mockResolvedValue(null);
    savePartyDraftMock.mockResolvedValue();
    loadCurrentAnalysisResultMock.mockResolvedValue(null);
    probeCodexEnvironmentMock.mockResolvedValue({
      codexPath: "C:\\codex.exe",
      codexVersion: "0.118.0",
      versionSupported: true,
      appServerInitialized: true,
      isolatedHome: "C:\\codex-home",
      platformFamily: "windows",
      platformOs: "windows",
      account: { authMode: null, planType: null, requiresOpenaiAuth: true },
      rateLimitsAvailable: false,
      diagnostics: [],
    });
  });

  it("凍結カタログ読み込み後に編成ビルダーを表示する", async () => {
    render(<App />);

    expect(screen.getByRole("status")).toHaveTextContent("凍結カタログ");
    expect(await screen.findByRole("heading", { name: "4人編成を作成" })).toBeInTheDocument();
    expect(screen.getByText("Ver.7.0 / 1キャラ")).toBeInTheDocument();
    expect(screen.getByRole("navigation", { name: "主要画面" })).toBeInTheDocument();
  });

  it("途中の編成を保存してサイドバーから開ける", async () => {
    const user = userEvent.setup();
    render(<App />);
    const name = await screen.findByRole("textbox", { name: "編成名" });

    await user.type(name, "蒸発チーム");
    await user.click(screen.getByRole("button", { name: "保存" }));

    expect(screen.getByText("「蒸発チーム」を保存しました。")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "蒸発チーム" })).toBeInTheDocument();
    expect(screen.getByText("1件")).toBeInTheDocument();
    expect(savePartyDraftMock).toHaveBeenCalledOnce();
  });

  it("編成の保存に成功すると作成フォームを新規状態へ戻す", async () => {
    const user = userEvent.setup();
    render(<App />);
    const name = await screen.findByRole("textbox", { name: "編成名" });

    await user.type(name, "保存後にリセット");
    await user.click(screen.getByRole("button", { name: "保存" }));

    expect(screen.getByRole("textbox", { name: "編成名" })).toHaveValue("");
    expect(screen.getByText("「保存後にリセット」を保存しました。")).toBeInTheDocument();
  });

  it("編成の保存に失敗すると入力を保持して再試行できる", async () => {
    const user = userEvent.setup();
    savePartyDraftMock.mockRejectedValueOnce(new Error("保存先に接続できません"));
    render(<App />);
    const name = await screen.findByRole("textbox", { name: "編成名" });

    await user.type(name, "保持する編成");
    await user.click(screen.getByRole("button", { name: "保存" }));

    expect(screen.getByRole("textbox", { name: "編成名" })).toHaveValue("保持する編成");
    expect(screen.getByText("保存先に接続できません")).toBeInTheDocument();
  });

  it("保存編成を選ぶと作成フォームを上書きせず分析結果へ移動する", async () => {
    const user = userEvent.setup();
    listPartyDraftsMock.mockResolvedValue([
      { partyId: "saved-party", name: "保存済み編成", currentResultId: null, updatedAt: "2026-08-31T00:00:00Z" },
    ]);
    loadPartyDraftMock.mockResolvedValue(savedParty);
    render(<App />);

    await user.click(await screen.findByRole("button", { name: "保存済み編成" }));

    expect(await screen.findByRole("heading", { name: "チーム分析結果" })).toBeInTheDocument();
    expect(screen.queryByRole("textbox", { name: "編成名" })).not.toBeInTheDocument();
    expect(screen.getByText(/保存済み編成はまだ分析されていません/)).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "保存済み編成" })).toHaveAttribute("aria-current", "page");
  });

  it("起動時の最新結果を読み込む間は未分析の空状態を重ねて表示しない", async () => {
    let resolveResult: (value: null) => void = () => undefined;
    loadCurrentAnalysisResultMock.mockImplementationOnce(
      () => new Promise<null>((resolve) => {
        resolveResult = resolve;
      }),
    );
    listPartyDraftsMock.mockResolvedValue([
      { partyId: "saved-party", name: "保存済み編成", currentResultId: "result-1", updatedAt: "2026-08-31T01:00:00Z" },
    ]);
    loadPartyDraftMock.mockResolvedValue(savedParty);
    render(<App />);

    expect(await screen.findByText("分析結果を読み込み中です。")).toBeInTheDocument();
    expect(screen.queryByTestId("team-result-empty")).not.toBeInTheDocument();

    await act(async () => resolveResult(null));

    expect(await screen.findByTestId("team-result-empty")).toHaveTextContent("保存済み編成はまだ分析されていません");
  });

  it("Codex設定からGate0を開ける", async () => {
    const user = userEvent.setup();
    render(<App />);

    await user.click(await screen.findByRole("button", { name: "Codex設定" }));

    expect(screen.getByRole("heading", { name: "Codexの環境を確認" })).toBeInTheDocument();
    expect(await screen.findByText("未連携")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "再確認" })).toBeEnabled();
  });

  it("カタログ読み込み失敗を表示して再実行できる", async () => {
    loadCatalogMock.mockRejectedValueOnce(new Error("カタログが壊れています"));
    const user = userEvent.setup();
    render(<App />);

    expect(await screen.findByRole("alert")).toHaveTextContent("カタログが壊れています");
    await user.click(screen.getByRole("button", { name: "再読み込み" }));

    expect(await screen.findByRole("heading", { name: "4人編成を作成" })).toBeInTheDocument();
    expect(loadCatalogMock).toHaveBeenCalledTimes(2);
  });
});
