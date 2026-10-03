import { AlertDialog } from "@base-ui/react/alert-dialog";
import { useEffect, useMemo, useRef, useState } from "react";
import {
  Database,
  PanelLeftClose,
  PanelLeftOpen,
  Plus,
  Search,
  Settings2,
  Trash2,
  Users,
} from "lucide-react";
import type { AnalysisMode, AnalysisStatus, ResultValidity, TeamBuildResolution } from "./domain/analysisTypes";
import type { Catalog } from "./domain/catalogTypes";
import {
  AnalysisNotesPanel,
  AnalysisProgressPanel,
  TeamResultPanel,
  buildAnalysisInput,
  cancelAnalysis,
  loadCurrentAnalysisResult,
  saveAnalysisVariantSelection,
  startAnalysis,
  subscribeAnalysisProgress,
  type AnalysisCharacterStepStatus,
  type CharacterAnalysisStep,
} from "./features/analysis";
import { loadCatalog } from "./features/catalog";
import { Gate0Screen } from "./features/gate0/Gate0Screen";
import { TavilySettingsPanel } from "./features/tavily/TavilySettingsPanel";
import {
  PartyBuilder,
  createEmptyParty,
  deletePartyDraft,
  listPartyDrafts,
  loadPartyDraft,
  savePartyDraft,
  type PartyDraft,
} from "./features/party";
import { cn } from "./lib/cn";

type WorkspaceView = "party" | "settings";

const NAVIGATION = [
  { id: "party" as const, label: "編成を作る", icon: Users },
  { id: "settings" as const, label: "調査設定", icon: Settings2 },
];

const ACTIVE_ANALYSIS_STATUSES = new Set<AnalysisStatus>([
  "queued",
  "starting_codex",
  "researching",
  "verifying_sources",
  "reconciling",
  "solving",
  "persisting",
]);

const CHARACTER_STAGES = new Set<AnalysisCharacterStepStatus>([
  "queued",
  "researching",
  "verifying",
  "reconciling",
  "solving",
  "completed",
  "failed",
  "cancelled",
]);

function createPartyId() {
  return globalThis.crypto?.randomUUID?.() ?? `party-${Date.now()}`;
}

function getPartyId(party: PartyDraft) {
  return party.partyId ?? party.id;
}

function LoadingScreen() {
  return (
    <main className="grid min-h-dvh place-items-center bg-slate-950 px-6 text-slate-100">
      <div className="max-w-md text-center" role="status" aria-live="polite">
        <p className="text-sm font-semibold text-amber-400">凍結カタログを検証中</p>
        <h1 className="mt-3 text-balance text-3xl font-bold">ビルドレコメンダーを準備しています</h1>
      </div>
    </main>
  );
}

function CatalogError({ message, onRetry }: { message: string; onRetry: () => void }) {
  return (
    <main className="grid min-h-dvh place-items-center bg-slate-950 px-6 text-slate-100">
      <section className="max-w-xl border-l-4 border-rose-400 pl-5" role="alert">
        <p className="text-sm font-semibold text-rose-300">起動を続行できません</p>
        <h1 className="mt-2 text-balance text-3xl font-bold">カタログの読み込みに失敗しました</h1>
        <p className="mt-3 text-pretty leading-7 text-slate-300">{message}</p>
        <button
          type="button"
          className="mt-6 min-h-11 rounded-lg bg-amber-400 px-5 py-3 font-semibold text-slate-950 hover:bg-amber-300"
          onClick={onRetry}
        >
          再読み込み
        </button>
      </section>
    </main>
  );
}

function DeletePartyButton({
  party,
  onDelete,
  disabled,
}: {
  party: PartyDraft;
  onDelete: (party: PartyDraft) => Promise<void>;
  disabled: boolean;
}) {
  const [open, setOpen] = useState(false);
  const [deleting, setDeleting] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const handleDelete = async () => {
    setDeleting(true);
    setError(null);
    try {
      await onDelete(party);
      setOpen(false);
    } catch (deleteError) {
      setError(deleteError instanceof Error ? deleteError.message : "編成を削除できませんでした。");
    } finally {
      setDeleting(false);
    }
  };

  return (
    <AlertDialog.Root
      open={open}
      onOpenChange={(nextOpen) => {
        setOpen(nextOpen);
        if (!nextOpen) setError(null);
      }}
    >
      <AlertDialog.Trigger
        className="grid min-h-11 min-w-11 shrink-0 place-items-center rounded-lg text-slate-400 hover:bg-rose-400/10 hover:text-rose-300 disabled:cursor-not-allowed disabled:opacity-40"
        aria-label={`「${party.name}」を削除`}
        title={`「${party.name}」を削除`}
        disabled={disabled}
      >
        <Trash2 aria-hidden="true" size={18} />
      </AlertDialog.Trigger>
      <AlertDialog.Portal>
        <AlertDialog.Backdrop className="fixed inset-0 z-40 min-h-dvh bg-slate-950/80" />
        <AlertDialog.Popup className="fixed left-1/2 top-1/2 z-50 flex w-[28rem] max-w-[calc(100vw-2rem)] -translate-x-1/2 -translate-y-1/2 flex-col gap-5 rounded-xl border border-slate-700 bg-slate-900 p-6 text-slate-100 shadow-2xl">
          <div>
            <AlertDialog.Title className="text-balance text-xl font-bold">この編成を削除しますか？</AlertDialog.Title>
            <AlertDialog.Description className="mt-2 text-pretty text-sm leading-6 text-slate-300">
              「{party.name}」を保存編成の一覧から削除します。検証済みの分析履歴は監査用に保持されます。
            </AlertDialog.Description>
          </div>
          {error ? (
            <p className="border-l-2 border-rose-400 pl-3 text-pretty text-sm text-rose-200" role="alert">
              {error}
            </p>
          ) : null}
          <div className="flex flex-wrap justify-end gap-3">
            <AlertDialog.Close
              className="inline-flex min-h-11 items-center justify-center rounded-lg border border-slate-600 px-4 py-2 font-semibold text-slate-200 hover:bg-slate-800 disabled:opacity-50"
              disabled={deleting}
            >
              キャンセル
            </AlertDialog.Close>
            <button
              type="button"
              className="inline-flex min-h-11 items-center justify-center rounded-lg bg-rose-500 px-4 py-2 font-semibold text-white hover:bg-rose-400 disabled:cursor-wait disabled:opacity-60"
              onClick={() => void handleDelete()}
              disabled={deleting}
            >
              {deleting ? "削除中" : "削除する"}
            </button>
          </div>
        </AlertDialog.Popup>
      </AlertDialog.Portal>
    </AlertDialog.Root>
  );
}

function Workspace({ catalog }: { catalog: Catalog }) {
  const [view, setView] = useState<WorkspaceView>("party");
  const [sidebarOpen, setSidebarOpen] = useState(true);
  const [draft, setDraft] = useState<PartyDraft>(() => createEmptyParty(createPartyId()));
  const [savedParties, setSavedParties] = useState<PartyDraft[]>([]);
  const [search, setSearch] = useState("");
  const [notice, setNotice] = useState("");
  const [analysisMode, setAnalysisMode] = useState<AnalysisMode>("normal");
  const [analysisStatus, setAnalysisStatus] = useState<AnalysisStatus>("queued");
  const [analysisPartyId, setAnalysisPartyId] = useState<string | null>(null);
  const [analysisError, setAnalysisError] = useState<string | null>(null);
  const [analysisErrorPartyId, setAnalysisErrorPartyId] = useState<string | null>(null);
  const [steps, setSteps] = useState<CharacterAnalysisStep[]>([]);
  const [resolution, setResolution] = useState<TeamBuildResolution | null>(null);
  const [resultValidity, setResultValidity] = useState<ResultValidity | null>(null);
  const [resultParty, setResultParty] = useState<PartyDraft | null>(null);
  const [selectedPartyId, setSelectedPartyId] = useState<string | null>(null);
  const [resultLoading, setResultLoading] = useState(false);
  const [resultError, setResultError] = useState<string | null>(null);
  const [choosingVariantKey, setChoosingVariantKey] = useState<string | null>(null);
  const resultLoadGeneration = useRef(0);
  const visiblePartyId = useRef<string | null>(null);
  const draftInteractionVersion = useRef(0);
  const analysisOperationPartyId = useRef<string | null>(null);
  const activeAnalysisRunId = useRef<string | null>(null);
  const variantSelectionPending = useRef<string | null>(null);

  const isAnalysisActive =
    analysisPartyId !== null && steps.length > 0 && ACTIVE_ANALYSIS_STATUSES.has(analysisStatus);
  const currentDraftId = getPartyId(draft) ?? null;
  const selectedIsSaved =
    selectedPartyId !== null && savedParties.some((party) => getPartyId(party) === selectedPartyId);
  const showAnalysisProgress =
    view === "party" && isAnalysisActive && currentDraftId === analysisPartyId;

  useEffect(() => {
    let active = true;
    const generation = ++resultLoadGeneration.current;
    const initialDraftVersion = draftInteractionVersion.current;
    const isCurrent = () => active && resultLoadGeneration.current === generation;

    (async () => {
      const summaries = await listPartyDrafts();
      const sortedSummaries = [...summaries].sort((left, right) => {
        const leftTime = Date.parse(left.updatedAt);
        const rightTime = Date.parse(right.updatedAt);
        if (Number.isNaN(leftTime) && Number.isNaN(rightTime)) return 0;
        if (Number.isNaN(leftTime)) return 1;
        if (Number.isNaN(rightTime)) return -1;
        return rightTime - leftTime;
      });
      const entries = await Promise.all(
        sortedSummaries.map(async (summary) => ({ summary, party: await loadPartyDraft(summary.partyId) })),
      );
      if (!isCurrent()) return;

      const parties = entries
        .map(({ party }) => party)
        .filter((party): party is PartyDraft => party !== null);
      setSavedParties(parties);
      const latestParty = parties[0];
      if (!latestParty) return;
      if (draftInteractionVersion.current !== initialDraftVersion || visiblePartyId.current !== null) return;

      const partyId = getPartyId(latestParty);
      if (!partyId) return;
      visiblePartyId.current = partyId;
      setDraft(latestParty);
      setSelectedPartyId(partyId);
      setResultParty(latestParty);
      setResultLoading(true);
      setResultError(null);
      const current = await loadCurrentAnalysisResult(partyId);
      if (!isCurrent()) return;
      setResolution(current);
      setResultValidity(current ? "current" : null);
      setAnalysisStatus(current ? "succeeded" : "queued");
      setResultLoading(false);
    })().catch((error: unknown) => {
      if (!isCurrent()) return;
      setResultLoading(false);
      setResultError(error instanceof Error ? error.message : String(error));
      setNotice(error instanceof Error ? error.message : "保存編成を読み込めませんでした。");
    });
    return () => {
      active = false;
    };
  }, []);

  const loadResultForParty = async (party: PartyDraft) => {
    const partyId = getPartyId(party);
    if (!partyId) return;
    const generation = ++resultLoadGeneration.current;
    draftInteractionVersion.current += 1;
    visiblePartyId.current = partyId;
    setView("party");
    setDraft(party);
    setSelectedPartyId(partyId);
    setResultParty(party);
    setResolution(null);
    setResultValidity(null);
    setResultError(null);
    setResultLoading(true);
    setAnalysisError(null);
    setAnalysisErrorPartyId(null);
    try {
      const current = await loadCurrentAnalysisResult(partyId);
      if (resultLoadGeneration.current !== generation || visiblePartyId.current !== partyId) return;
      setResolution(current);
      setResultValidity(current ? (analysisPartyId === partyId ? "soft_stale" : "current") : null);
    } catch (error: unknown) {
      if (resultLoadGeneration.current !== generation || visiblePartyId.current !== partyId) return;
      setResultError(error instanceof Error ? error.message : String(error));
      setNotice(error instanceof Error ? error.message : "分析結果を読み込めませんでした。");
    } finally {
      if (resultLoadGeneration.current === generation && visiblePartyId.current === partyId) {
        setResultLoading(false);
      }
    }
  };

  const filteredParties = useMemo(() => {
    const query = search.trim().toLocaleLowerCase("ja");
    return query.length === 0
      ? savedParties
      : savedParties.filter((party) => party.name.toLocaleLowerCase("ja").includes(query));
  }, [savedParties, search]);

  const handleAnalyze = async (next: PartyDraft) => {
    const characterById = new Map(catalog.characters.map((character) => [character.id, character]));
    const partyId = getPartyId(next) ?? createPartyId();
    const saved = { ...next, partyId, id: partyId };
    const wasPreviouslySaved = savedParties.some((party) => getPartyId(party) === partyId);
    let temporaryDraftSaved = false;
    let unlisten: () => void = () => undefined;

    try {
      const input = buildAnalysisInput(saved, catalog);
      analysisOperationPartyId.current = partyId;
      activeAnalysisRunId.current = null;
      resultLoadGeneration.current += 1;
      draftInteractionVersion.current += 1;
      visiblePartyId.current = partyId;
      setView("party");
      setDraft(saved);
      setSelectedPartyId(partyId);
      setAnalysisPartyId(partyId);
      setAnalysisStatus("queued");
      setAnalysisError(null);
      setAnalysisErrorPartyId(null);
      setResultError(null);
      setResultLoading(false);
      setSteps(
        saved.members.map((member) => {
          const character = member.characterId ? characterById.get(member.characterId) : undefined;
          return {
            characterId: member.characterId ?? `slot-${member.slotIndex + 1}`,
            characterName: character?.name,
            characterImageUrl: character?.imageUrl,
            status: "queued",
          };
        }),
      );
      setNotice("編成を受け付けました。分析の準備をしています。");

      await savePartyDraft(saved);
      temporaryDraftSaved = true;
      if (wasPreviouslySaved) {
        setSavedParties((current) => [saved, ...current.filter((party) => getPartyId(party) !== partyId)]);
        setResultValidity(resolution ? "soft_stale" : null);
      } else {
        setResultParty(saved);
        setResolution(null);
        setResultValidity(null);
      }
      setNotice("編成を受け付けました。4人の情報を順番に確認しています。");

      unlisten = await subscribeAnalysisProgress((progress) => {
        if (activeAnalysisRunId.current === null) {
          if (progress.status !== "starting_codex") return;
          activeAnalysisRunId.current = progress.analysisRunId;
        }
        if (activeAnalysisRunId.current !== progress.analysisRunId) return;
        setAnalysisStatus(progress.status);
        if (progress.error) {
          setAnalysisError(progress.error);
          setAnalysisErrorPartyId(partyId);
        }
        if (progress.status === "cancelled") {
          setSteps((current) =>
            current.map((step) => ({
              ...step,
              status: "cancelled",
            })),
          );
        }
        if (
          progress.characterId &&
          progress.characterStage &&
          CHARACTER_STAGES.has(progress.characterStage as AnalysisCharacterStepStatus)
        ) {
          setSteps((current) =>
            current.map((step) =>
              step.characterId === progress.characterId
                ? {
                    ...step,
                    status: progress.characterStage as AnalysisCharacterStepStatus,
                    detail: progress.detail,
                    error: progress.error ?? undefined,
                  }
                : step,
            ),
          );
        }
      });

      const completed = await startAnalysis(input, analysisMode);
      setSavedParties((current) => [saved, ...current.filter((party) => getPartyId(party) !== partyId)]);
      setAnalysisStatus("succeeded");
      analysisOperationPartyId.current = null;
      activeAnalysisRunId.current = null;
      setAnalysisPartyId(null);
      setSteps([]);
      setAnalysisError(null);
      setAnalysisErrorPartyId(null);
      if (visiblePartyId.current === partyId) {
        resultLoadGeneration.current += 1;
        setResultLoading(false);
        setResultError(null);
        if (wasPreviouslySaved) {
          setResolution(completed.resolution);
          setResultParty(saved);
          setSelectedPartyId(partyId);
          setResultValidity("current");
        } else {
          visiblePartyId.current = null;
          setDraft(createEmptyParty(createPartyId()));
          setResolution(null);
          setResultParty(null);
          setSelectedPartyId(null);
          setResultValidity(null);
        }
      }
      setNotice(`「${saved.name}」の分析結果を保存しました。`);
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      const wasCancelled = message.includes("キャンセル") || message.toLocaleLowerCase().includes("cancel");
      setAnalysisStatus(wasCancelled ? "cancelled" : "failed");
      analysisOperationPartyId.current = null;
      activeAnalysisRunId.current = null;
      setAnalysisPartyId(null);
      setSteps([]);
      setAnalysisError(message);
      setAnalysisErrorPartyId(partyId);
      if (!wasPreviouslySaved && temporaryDraftSaved) {
        try {
          await deletePartyDraft(partyId);
        } catch (deleteError) {
          const cleanupMessage = deleteError instanceof Error ? deleteError.message : String(deleteError);
          setNotice(`${message} 一時保存データの整理にも失敗しました: ${cleanupMessage}`);
          return;
        }
      }
      setNotice(message);
    } finally {
      unlisten();
    }
  };

  const handleNewParty = () => {
    resultLoadGeneration.current += 1;
    draftInteractionVersion.current += 1;
    visiblePartyId.current = null;
    setResultLoading(false);
    setResultError(null);
    setResultParty(null);
    setResolution(null);
    setResultValidity(null);
    setSelectedPartyId(null);
    setAnalysisError(null);
    setAnalysisErrorPartyId(null);
    setDraft(createEmptyParty(createPartyId()));
    setNotice("新しい編成を開きました。");
    setView("party");
  };

  const handleDeleteParty = async (party: PartyDraft) => {
    const partyId = getPartyId(party);
    if (!partyId) throw new Error("削除対象の編成IDがありません。");
    if (analysisOperationPartyId.current === partyId) {
      throw new Error("分析中の編成は削除できません。分析をキャンセルしてから削除してください。");
    }
    await deletePartyDraft(partyId);
    setSavedParties((current) => current.filter((candidate) => getPartyId(candidate) !== partyId));
    if (visiblePartyId.current === partyId) {
      handleNewParty();
    }
    setNotice(`「${party.name}」を削除しました。`);
  };

  const handleChooseVariant = (characterId: string, variantId: string) => {
    const partyId = resultParty ? getPartyId(resultParty) : undefined;
    if (!partyId) return;
    const selectionKey = `${characterId}:${variantId}`;
    if (variantSelectionPending.current !== null) return;
    variantSelectionPending.current = selectionKey;
    setChoosingVariantKey(selectionKey);
    const generation = resultLoadGeneration.current;
    void saveAnalysisVariantSelection(partyId, characterId, variantId)
      .then((selected) => {
        if (
          resultLoadGeneration.current !== generation ||
          visiblePartyId.current !== partyId ||
          (resultParty ? getPartyId(resultParty) : undefined) !== partyId
        ) {
          return;
        }
        setResolution(selected);
        setResultValidity("current");
        setNotice("候補の選択を保存しました。");
      })
      .catch((error: unknown) => {
        if (visiblePartyId.current === partyId) {
          setNotice(error instanceof Error ? error.message : String(error));
        }
      })
      .finally(() => {
        if (variantSelectionPending.current === selectionKey) {
          variantSelectionPending.current = null;
          setChoosingVariantKey(null);
        }
      });
  };

  return (
    <div className="min-h-dvh bg-slate-950 text-slate-100">
      <a
        href="#workspace-main"
        className="sr-only z-50 rounded bg-amber-400 px-4 py-2 font-semibold text-slate-950 focus:not-sr-only focus:fixed focus:left-4 focus:top-4"
      >
        メインコンテンツへ
      </a>
      <div
        className={cn(
          "grid min-h-dvh",
          sidebarOpen
            ? view === "settings"
              ? "lg:grid-cols-[17rem_minmax(0,1fr)]"
              : "lg:grid-cols-[17rem_minmax(0,1fr)_19rem]"
            : view === "settings"
              ? "lg:grid-cols-[minmax(0,1fr)]"
              : "lg:grid-cols-[minmax(0,1fr)_19rem]",
        )}
      >
        <aside
          id="workspace-sidebar"
          className="border-b border-slate-800 bg-slate-900/55 p-5 lg:border-b-0 lg:border-r"
          aria-label="編成ナビゲーション"
          hidden={!sidebarOpen}
        >
            <div className="flex items-start justify-between gap-3">
              <div>
                <p className="text-xs font-semibold uppercase tracking-[0.18em] text-amber-400">ビルドレコメンダー</p>
                <h1 className="mt-2 text-balance text-xl font-bold">根拠付き編成ビルド</h1>
                <p className="mt-2 text-sm leading-6 text-slate-400">
                  Ver.{catalog.gameVersion} / {catalog.characters.length}キャラ
                </p>
              </div>
              <button
                type="button"
                className="grid min-h-11 min-w-11 place-items-center rounded-lg text-slate-300 hover:bg-slate-800 hover:text-slate-100"
                onClick={() => setSidebarOpen(false)}
                aria-label="サイドバーを閉じる"
                title="サイドバーを閉じる"
                aria-controls="workspace-sidebar"
                aria-expanded="true"
              >
                <PanelLeftClose aria-hidden="true" size={20} />
              </button>
            </div>

            <nav className="mt-6 grid gap-2 sm:grid-cols-2 lg:grid-cols-1" aria-label="主要画面">
              {NAVIGATION.map(({ id, label, icon: Icon }) => (
                <button
                  key={id}
                  type="button"
                  className={cn(
                    "flex min-h-11 items-center gap-3 rounded-lg px-3 py-2 text-left font-semibold disabled:cursor-not-allowed disabled:opacity-50",
                    view === id
                      ? "bg-slate-800 text-amber-300"
                      : "text-slate-300 hover:bg-slate-900 hover:text-slate-100",
                  )}
                  onClick={() => {
                    if (id === "party") handleNewParty();
                    else setView(id);
                  }}
                  disabled={isAnalysisActive}
                  aria-current={view === id && (id !== "party" || selectedPartyId === null) ? "page" : undefined}
                >
                  <Icon aria-hidden="true" size={19} strokeWidth={1.8} />
                  {label}
                </button>
              ))}
            </nav>

            <section className="mt-8 border-t border-slate-800 pt-5" aria-labelledby="saved-parties-heading">
              <div className="flex items-center justify-between gap-3">
                <h2 id="saved-parties-heading" className="font-semibold">保存した編成</h2>
                <button
                  type="button"
                  className="grid min-h-11 min-w-11 place-items-center rounded-lg text-slate-300 hover:bg-slate-800 hover:text-slate-100"
                  onClick={handleNewParty}
                  aria-label="新しい編成を作成"
                  title="新しい編成を作成"
                  disabled={isAnalysisActive}
                >
                  <Plus aria-hidden="true" size={20} />
                </button>
              </div>
              <label htmlFor="party-search" className="sr-only">
                保存編成を検索
              </label>
              <div className="relative mt-3">
                <Search
                  aria-hidden="true"
                  className="pointer-events-none absolute left-3 top-3 text-slate-500"
                  size={18}
                />
                <input
                  id="party-search"
                  type="search"
                  value={search}
                  onChange={(event) => setSearch(event.target.value)}
                  className="min-h-11 w-full rounded-lg border border-slate-700 bg-slate-950 py-2 pl-10 pr-3 text-sm placeholder:text-slate-500"
                  placeholder="編成名で検索"
                />
              </div>
              <div className="mt-3 space-y-1">
                {filteredParties.length === 0 ? (
                  <p className="text-sm leading-6 text-slate-500">保存した編成はまだありません。</p>
                ) : (
                  filteredParties.map((party) => {
                    const partyId = getPartyId(party);
                    const isSelected = view === "party" && selectedPartyId === partyId;
                    return (
                      <div key={partyId} className="flex min-w-0 items-center gap-1">
                        <button
                          type="button"
                          className={cn(
                            "min-h-11 min-w-0 flex-1 truncate rounded-lg px-3 py-2 text-left text-sm hover:bg-slate-800 hover:text-slate-100 disabled:cursor-not-allowed disabled:opacity-50",
                            isSelected ? "bg-slate-800 text-amber-300" : "text-slate-300",
                          )}
                          onClick={() => void loadResultForParty(party)}
                          aria-current={isSelected ? "page" : undefined}
                          disabled={isAnalysisActive}
                        >
                          {party.name}
                        </button>
                        <DeletePartyButton
                          party={party}
                          onDelete={handleDeleteParty}
                          disabled={isAnalysisActive && analysisPartyId === partyId}
                        />
                      </div>
                    );
                  })
                )}
              </div>
            </section>
        </aside>

        <main id="workspace-main" className="min-w-0 px-5 py-7 sm:px-8 lg:px-10 lg:py-9">
          {!sidebarOpen ? (
            <button
              type="button"
              className="mb-5 grid min-h-11 min-w-11 place-items-center rounded-lg border border-slate-700 text-slate-300 hover:bg-slate-900 hover:text-slate-100"
              onClick={() => setSidebarOpen(true)}
              aria-label="サイドバーを開く"
              title="サイドバーを開く"
              aria-controls="workspace-sidebar"
              aria-expanded="false"
            >
              <PanelLeftOpen aria-hidden="true" size={20} />
            </button>
          ) : null}
          {notice ? (
            <p
              className="mb-5 border-l-2 border-amber-400 pl-3 text-sm leading-6 text-slate-300"
              role="status"
              aria-live="polite"
            >
              {notice}
            </p>
          ) : null}

          {view === "party" ? (
            <div className="space-y-9">
              <PartyBuilder
                catalog={catalog}
                draft={draft}
                onChange={(nextDraft) => {
                  draftInteractionVersion.current += 1;
                  setDraft(nextDraft);
                }}
                onAnalyze={(next) => void handleAnalyze(next)}
                analysisMode={analysisMode}
                onAnalysisModeChange={setAnalysisMode}
                mode={selectedIsSaved ? "edit" : "create"}
                actionError={analysisErrorPartyId === currentDraftId ? analysisError : null}
                disabled={isAnalysisActive}
              />
              {showAnalysisProgress ? (
                <AnalysisProgressPanel
                  status={analysisStatus}
                  characterSteps={steps}
                  onCancel={() => {
                    void cancelAnalysis().catch((error: unknown) => {
                      setAnalysisError(error instanceof Error ? error.message : String(error));
                      setAnalysisErrorPartyId(analysisPartyId);
                    });
                  }}
                />
              ) : null}
              {selectedIsSaved ? (
                <section className="space-y-5" aria-label="保存編成の分析結果">
                  {resultLoading ? (
                    <p className="text-sm text-slate-400" role="status" aria-live="polite">
                      分析結果を読み込み中です。
                    </p>
                  ) : null}
                  {resultError ? (
                    <p className="border-l-2 border-rose-400 pl-3 text-sm leading-6 text-rose-200" role="alert">
                      {resultError}
                    </p>
                  ) : null}
                  {!resultLoading && !resultError ? (
                    <TeamResultPanel
                      catalog={catalog}
                      party={resultParty}
                      resolution={resolution}
                      validity={resultValidity ?? "current"}
                      onChooseVariant={handleChooseVariant}
                      choosingVariantKey={choosingVariantKey}
                    />
                  ) : null}
                </section>
              ) : null}
            </div>
          ) : null}
          {view === "settings" ? (
            <div className="space-y-8">
              <TavilySettingsPanel />
              <Gate0Screen embedded />
            </div>
          ) : null}
        </main>

        {view === "party" ? (
          <aside
            className="border-t border-slate-800 bg-slate-900/30 p-5 lg:border-l lg:border-t-0"
            aria-label={selectedIsSaved ? "分析結果の補足事項" : "現在の編成情報"}
          >
            {selectedIsSaved ? (
            <AnalysisNotesPanel catalog={catalog} resolution={resolution} validity={resultValidity ?? "current"} />
          ) : (
            <>
              <div className="flex items-center gap-3 text-slate-300">
                <Database aria-hidden="true" size={20} />
                <h2 className="font-semibold">データ状態</h2>
              </div>
              <dl className="mt-4 space-y-4 text-sm">
                <div>
                  <dt className="text-slate-500">カタログ</dt>
                  <dd className="mt-1 break-words text-slate-200">{catalog.schemaVersion}</dd>
                </div>
                <div>
                  <dt className="text-slate-500">更新日</dt>
                  <dd className="mt-1 tabular-nums text-slate-200">{catalog.catalogUpdatedAt}</dd>
                </div>
                <div>
                  <dt className="text-slate-500">編成</dt>
                  <dd className="mt-1 break-words text-slate-200">{draft.name.trim() || "未命名の編成"}</dd>
                </div>
                <div>
                  <dt className="text-slate-500">保存件数</dt>
                  <dd className="mt-1 tabular-nums text-slate-200">{savedParties.length}件</dd>
                </div>
              </dl>
              <p className="mt-6 border-t border-slate-800 pt-5 text-pretty text-xs leading-5 text-slate-500">
                カタログと推薦根拠は分離して保存します。根拠の検証に合格した結果だけが現在結果になります。
              </p>
            </>
            )}
          </aside>
        ) : null}
      </div>
    </div>
  );
}

function App() {
  const [attempt, setAttempt] = useState(0);
  const [state, setState] = useState<
    | { status: "loading" }
    | { status: "ready"; catalog: Catalog }
    | { status: "error"; message: string }
  >({ status: "loading" });

  useEffect(() => {
    let active = true;
    loadCatalog()
      .then((catalog) => {
        if (active) setState({ status: "ready", catalog });
      })
      .catch((error: unknown) => {
        if (!active) return;
        setState({
          status: "error",
          message: error instanceof Error ? error.message : "カタログの検証に失敗しました。",
        });
      });
    return () => {
      active = false;
    };
  }, [attempt]);

  if (state.status === "loading") return <LoadingScreen />;
  if (state.status === "error") {
    return (
      <CatalogError
        message={state.message}
        onRetry={() => {
          setState({ status: "loading" });
          setAttempt((value) => value + 1);
        }}
      />
    );
  }
  return <Workspace catalog={state.catalog} />;
}

export default App;
