import "@testing-library/jest-dom/vitest";
import { act, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import App from "./App";
import type { Catalog } from "./domain/catalogTypes";
import { loadCurrentAnalysisResult, startAnalysis, subscribeAnalysisProgress } from "./features/analysis";
import type { AnalysisProgressEvent } from "./features/analysis";
import { loadCatalog } from "./features/catalog";
import { probeCodexEnvironment } from "./features/gate0/probeCodexEnvironment";
import { deletePartyDraft, listPartyDrafts, loadPartyDraft, savePartyDraft } from "./features/party";
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
  startAnalysis: vi.fn(),
  subscribeAnalysisProgress: vi.fn(),
}));

vi.mock("./features/party", async (importOriginal) => ({
  ...(await importOriginal<typeof import("./features/party")>()),
  listPartyDrafts: vi.fn(),
  loadPartyDraft: vi.fn(),
  savePartyDraft: vi.fn(),
  deletePartyDraft: vi.fn(),
}));

const loadCatalogMock = vi.mocked(loadCatalog);
const probeCodexEnvironmentMock = vi.mocked(probeCodexEnvironment);
const listPartyDraftsMock = vi.mocked(listPartyDrafts);
const loadPartyDraftMock = vi.mocked(loadPartyDraft);
const savePartyDraftMock = vi.mocked(savePartyDraft);
const deletePartyDraftMock = vi.mocked(deletePartyDraft);
const loadCurrentAnalysisResultMock = vi.mocked(loadCurrentAnalysisResult);
const startAnalysisMock = vi.mocked(startAnalysis);
const subscribeAnalysisProgressMock = vi.mocked(subscribeAnalysisProgress);
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
    { id: "char-b", name: "キャラB", element: "水", weaponType: "片手剣", rarity: 5, imageUrl: "b.png" },
    { id: "char-c", name: "キャラC", element: "風", weaponType: "片手剣", rarity: 4, imageUrl: "c.png" },
    { id: "char-d", name: "キャラD", element: "雷", weaponType: "片手剣", rarity: 4, imageUrl: "d.png" },
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

const validSavedParty: PartyDraft = {
  ...savedParty,
  members: ["char-a", "char-b", "char-c", "char-d"].map((characterId, slotIndex) => ({
    slotIndex: slotIndex as 0 | 1 | 2 | 3,
    characterId,
    weaponId: "weapon-a",
    constellation: 0,
    refinement: 1,
  })) as PartyDraft["members"],
};

async function fillValidParty(user: ReturnType<typeof userEvent.setup>, name: string) {
  await user.type(screen.getByRole("textbox", { name: "編成名" }), name);
  const characterSelects = screen.getAllByRole("combobox", { name: "キャラクター" });
  const weaponSelects = screen.getAllByRole("combobox", { name: "武器" });
  for (const [index, characterId] of ["char-a", "char-b", "char-c", "char-d"].entries()) {
    await user.selectOptions(characterSelects[index], characterId);
    await user.selectOptions(weaponSelects[index], "weapon-a");
  }
}

describe("アプリワークスペース", () => {
  beforeEach(() => {
    window.__TAURI_INTERNALS__ = {};
    vi.clearAllMocks();
    loadCatalogMock.mockResolvedValue(catalog);
    listPartyDraftsMock.mockResolvedValue([]);
    loadPartyDraftMock.mockResolvedValue(null);
    savePartyDraftMock.mockResolvedValue();
    deletePartyDraftMock.mockResolvedValue();
    loadCurrentAnalysisResultMock.mockResolvedValue(null);
    subscribeAnalysisProgressMock.mockResolvedValue(() => undefined);
    startAnalysisMock.mockResolvedValue({ resolution: null } as never);
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

  afterEach(() => {
    delete window.__TAURI_INTERNALS__;
  });

  it("凍結カタログ読み込み後に編成ビルダーを表示する", async () => {
    render(<App />);

    expect(screen.getByRole("status")).toHaveTextContent("凍結カタログ");
    expect(await screen.findByRole("heading", { name: "4人編成を作成" })).toBeInTheDocument();
    expect(screen.getByText("Ver.7.0 / 4キャラ")).toBeInTheDocument();
    expect(screen.getByRole("navigation", { name: "主要画面" })).toBeInTheDocument();
  });

  it("保存ボタンと独立した分析結果ページを表示しない", async () => {
    render(<App />);

    await screen.findByRole("heading", { name: "4人編成を作成" });
    expect(screen.queryByRole("button", { name: "保存" })).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "分析結果" })).not.toBeInTheDocument();
  });

  it("起動中に始めた新規入力を、遅れて読めた保存編成で上書きしない", async () => {
    const user = userEvent.setup();
    let resolveSavedParty: (party: PartyDraft | null) => void = () => undefined;
    listPartyDraftsMock.mockResolvedValue([
      { partyId: "saved-party", name: "保存済み編成", currentResultId: null, updatedAt: "2026-08-31T00:00:00Z" },
    ]);
    loadPartyDraftMock.mockImplementationOnce(
      () => new Promise<PartyDraft | null>((resolve) => {
        resolveSavedParty = resolve;
      }),
    );
    render(<App />);

    const nameInput = await screen.findByRole("textbox", { name: "編成名" });
    await user.type(nameInput, "入力中の新規編成");
    await act(async () => resolveSavedParty(savedParty));

    expect(screen.getByRole("textbox", { name: "編成名" })).toHaveValue("入力中の新規編成");
    expect(await screen.findByRole("button", { name: "保存済み編成" })).toBeInTheDocument();
  });

  it("分析成功後に保存編成へ追加する", async () => {
    const user = userEvent.setup();
    render(<App />);
    await screen.findByRole("textbox", { name: "編成名" });
    await fillValidParty(user, "蒸発チーム");
    await user.click(screen.getByRole("button", { name: "分析を開始" }));

    expect(await screen.findByText("「蒸発チーム」の分析結果を保存しました。")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "蒸発チーム" })).toBeInTheDocument();
    expect(screen.getByRole("textbox", { name: "編成名" })).toHaveValue("");
    expect(savePartyDraftMock).toHaveBeenCalledOnce();
    expect(startAnalysisMock).toHaveBeenCalledOnce();
    expect(startAnalysisMock).toHaveBeenCalledWith(expect.any(Object), "normal");
    expect(savePartyDraftMock.mock.calls[0]?.[0]).not.toHaveProperty("analysisMode");
  });

  it("高速モードを選ぶと分析呼び出しへ高速を渡す", async () => {
    const user = userEvent.setup();
    render(<App />);
    await screen.findByRole("textbox", { name: "編成名" });
    await fillValidParty(user, "高速チーム");

    expect(screen.getByRole("radio", { name: "通常" })).toBeChecked();
    await user.click(screen.getByRole("radio", { name: "高速" }));
    expect(screen.getByRole("radio", { name: "高速" })).toBeChecked();
    await user.click(screen.getByRole("button", { name: "分析を開始" }));

    expect(await screen.findByText("「高速チーム」の分析結果を保存しました。")).toBeInTheDocument();
    expect(startAnalysisMock).toHaveBeenCalledWith(expect.any(Object), "fast");
  });

  it("分析失敗時は新規編成を一覧へ加えず入力を保持する", async () => {
    const user = userEvent.setup();
    startAnalysisMock.mockRejectedValueOnce(new Error("分析サービスに接続できません"));
    render(<App />);

    await screen.findByRole("textbox", { name: "編成名" });
    await fillValidParty(user, "再試行する編成");
    await user.click(screen.getByRole("button", { name: "分析を開始" }));

    expect(await screen.findByRole("alert")).toHaveTextContent("分析サービスに接続できません");
    expect(screen.getByRole("textbox", { name: "編成名" })).toHaveValue("再試行する編成");
    expect(screen.queryByRole("button", { name: "再試行する編成" })).not.toBeInTheDocument();
    expect(deletePartyDraftMock).toHaveBeenCalledOnce();
  });

  it("保存編成を選ぶと編集フォームと分析結果を同じ画面へ表示する", async () => {
    const user = userEvent.setup();
    listPartyDraftsMock.mockResolvedValue([
      { partyId: "saved-party", name: "保存済み編成", currentResultId: null, updatedAt: "2026-08-31T00:00:00Z" },
    ]);
    loadPartyDraftMock.mockResolvedValue(savedParty);
    render(<App />);

    await user.click(await screen.findByRole("button", { name: "保存済み編成" }));

    expect(await screen.findByRole("heading", { name: "チーム分析結果" })).toBeInTheDocument();
    expect(screen.getByRole("textbox", { name: "編成名" })).toHaveValue("保存済み編成");
    expect(screen.getByRole("button", { name: "分析を更新" })).toBeInTheDocument();
    expect(screen.getByText(/保存済み編成はまだ分析されていません/)).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "保存済み編成" })).toHaveAttribute("aria-current", "page");
  });

  it("保存編成を編集して分析を更新する", async () => {
    const user = userEvent.setup();
    listPartyDraftsMock.mockResolvedValue([
      { partyId: "saved-party", name: "保存済み編成", currentResultId: null, updatedAt: "2026-08-31T00:00:00Z" },
    ]);
    loadPartyDraftMock.mockResolvedValue(validSavedParty);
    render(<App />);

    await screen.findByRole("button", { name: "分析を更新" });
    const nameInput = await screen.findByRole("textbox", { name: "編成名" });
    await user.clear(nameInput);
    await user.type(nameInput, "更新後の編成");
    await user.click(screen.getByRole("button", { name: "分析を更新" }));

    expect(await screen.findByText("「更新後の編成」の分析結果を保存しました。")).toBeInTheDocument();
    expect(screen.getByRole("textbox", { name: "編成名" })).toHaveValue("更新後の編成");
    expect(screen.getByRole("button", { name: "更新後の編成" })).toBeInTheDocument();
    expect(savePartyDraftMock).toHaveBeenCalledWith(expect.objectContaining({ partyId: "saved-party", name: "更新後の編成" }));
  });

  it("再分析の保存開始から完了まで画面移動と対象削除を無効にする", async () => {
    const user = userEvent.setup();
    let resolveSave: () => void = () => undefined;
    let resolveAnalysis: (value: never) => void = () => undefined;
    savePartyDraftMock.mockImplementationOnce(
      () => new Promise<void>((resolve) => {
        resolveSave = resolve;
      }),
    );
    startAnalysisMock.mockImplementationOnce(
      () => new Promise((resolve) => {
        resolveAnalysis = resolve;
      }),
    );
    listPartyDraftsMock.mockResolvedValue([
      { partyId: "saved-party", name: "保存済み編成", currentResultId: null, updatedAt: "2026-08-31T00:00:00Z" },
    ]);
    loadPartyDraftMock.mockResolvedValue(validSavedParty);
    render(<App />);

    await user.click(await screen.findByRole("button", { name: "分析を更新" }));

    expect(screen.getByRole("button", { name: "編成を作る" })).toBeDisabled();
    expect(screen.getByRole("button", { name: "調査設定" })).toBeDisabled();
    expect(screen.getByRole("button", { name: "保存済み編成" })).toBeDisabled();
    expect(screen.getByRole("button", { name: "「保存済み編成」を削除" })).toBeDisabled();
    expect(screen.getByTestId("analysis-progress-panel")).toBeInTheDocument();

    await act(async () => resolveSave());
    await act(async () => resolveAnalysis({ resolution: null } as never));
  });

  it("現在の実行IDと異なる分析進捗イベントを無視する", async () => {
    const user = userEvent.setup();
    let emitProgress: (event: AnalysisProgressEvent) => void = () => undefined;
    let resolveAnalysis: (value: never) => void = () => undefined;
    subscribeAnalysisProgressMock.mockImplementationOnce(async (onProgress) => {
      emitProgress = onProgress;
      return () => undefined;
    });
    startAnalysisMock.mockImplementationOnce(
      () => new Promise((resolve) => {
        resolveAnalysis = resolve;
      }),
    );
    listPartyDraftsMock.mockResolvedValue([
      { partyId: "saved-party", name: "保存済み編成", currentResultId: null, updatedAt: "2026-08-31T00:00:00Z" },
    ]);
    loadPartyDraftMock.mockResolvedValue(validSavedParty);
    render(<App />);

    await user.click(await screen.findByRole("button", { name: "分析を更新" }));
    await act(async () => {
      emitProgress({
        analysisRunId: "current-run",
        status: "starting_codex",
        characterId: null,
        characterStage: null,
        detail: "開始",
        error: null,
      });
    });
    expect(screen.getByTestId("analysis-progress-panel")).toHaveTextContent("分析を開始中");

    await act(async () => {
      emitProgress({
        analysisRunId: "old-run",
        status: "failed",
        characterId: null,
        characterStage: null,
        detail: "古い失敗",
        error: "古い実行のエラー",
      });
    });
    expect(screen.getByTestId("analysis-progress-panel")).toHaveTextContent("分析を開始中");
    expect(screen.queryByText("古い実行のエラー")).not.toBeInTheDocument();

    await act(async () => resolveAnalysis({ resolution: null } as never));
  });

  it("サイドバーをボタンで閉じて再表示できる", async () => {
    const user = userEvent.setup();
    render(<App />);

    await user.click(await screen.findByRole("button", { name: "サイドバーを閉じる" }));
    expect(screen.queryByRole("complementary", { name: "編成ナビゲーション" })).not.toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "サイドバーを開く" }));
    expect(screen.getByRole("complementary", { name: "編成ナビゲーション" })).toBeInTheDocument();
  });

  it("保存編成を確認ダイアログから削除する", async () => {
    const user = userEvent.setup();
    listPartyDraftsMock.mockResolvedValue([
      { partyId: "saved-party", name: "保存済み編成", currentResultId: null, updatedAt: "2026-08-31T00:00:00Z" },
    ]);
    loadPartyDraftMock.mockResolvedValue(savedParty);
    render(<App />);

    await user.click(await screen.findByRole("button", { name: "「保存済み編成」を削除" }));
    expect(screen.getByRole("alertdialog")).toHaveTextContent("この編成を削除しますか？");
    await user.click(screen.getByRole("button", { name: "削除する" }));

    expect(deletePartyDraftMock).toHaveBeenCalledWith("saved-party");
    expect(screen.queryByRole("button", { name: "保存済み編成" })).not.toBeInTheDocument();
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

  it("調査設定からGate0を開ける", async () => {
    const user = userEvent.setup();
    render(<App />);

    await user.click(await screen.findByRole("button", { name: "調査設定" }));

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
