import { useEffect, useMemo, useState } from "react";
import { BarChart3, Database, Plus, Search, Settings2, Users } from "lucide-react";
import type { AnalysisStatus } from "./domain/analysisTypes";
import type { Catalog } from "./domain/catalogTypes";
import {
  AnalysisProgressPanel,
  TeamResultPanel,
  type CharacterAnalysisStep,
} from "./features/analysis";
import { loadCatalog } from "./features/catalog";
import { Gate0Screen } from "./features/gate0/Gate0Screen";
import { PartyBuilder, createEmptyParty, type PartyDraft } from "./features/party";
import { cn } from "./lib/cn";

type WorkspaceView = "party" | "analysis" | "settings";

const NAVIGATION = [
  { id: "party" as const, label: "編成を作る", icon: Users },
  { id: "analysis" as const, label: "分析結果", icon: BarChart3 },
  { id: "settings" as const, label: "Codex設定", icon: Settings2 },
];

function createPartyId() {
  return globalThis.crypto?.randomUUID?.() ?? `party-${Date.now()}`;
}

function LoadingScreen() {
  return (
    <main className="grid min-h-dvh place-items-center bg-slate-950 px-6 text-slate-100">
      <div className="max-w-md text-center" role="status" aria-live="polite">
        <p className="text-sm font-semibold text-amber-400">凍結カタログを検証中</p>
        <h1 className="mt-3 text-balance text-3xl font-bold">原神ビルド推薦を準備しています</h1>
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

function Workspace({ catalog }: { catalog: Catalog }) {
  const [view, setView] = useState<WorkspaceView>("party");
  const [draft, setDraft] = useState<PartyDraft>(() => createEmptyParty(createPartyId()));
  const [savedParties, setSavedParties] = useState<PartyDraft[]>([]);
  const [search, setSearch] = useState("");
  const [notice, setNotice] = useState("");
  const [analysisStatus, setAnalysisStatus] = useState<AnalysisStatus>("queued");
  const [steps, setSteps] = useState<CharacterAnalysisStep[]>([]);

  const filteredParties = useMemo(() => {
    const query = search.trim().toLocaleLowerCase("ja");
    return query.length === 0
      ? savedParties
      : savedParties.filter((party) => party.name.toLocaleLowerCase("ja").includes(query));
  }, [savedParties, search]);

  const handleSave = (next: PartyDraft) => {
    const partyId = next.partyId ?? next.id ?? createPartyId();
    const saved = { ...next, partyId, id: partyId };
    setDraft(saved);
    setSavedParties((current) => {
      const remaining = current.filter((party) => (party.partyId ?? party.id) !== partyId);
      return [saved, ...remaining];
    });
    setNotice(`「${saved.name}」を保存しました。`);
  };

  const handleAnalyze = (next: PartyDraft) => {
    const characterById = new Map(catalog.characters.map((character) => [character.id, character]));
    setDraft(next);
    setAnalysisStatus("queued");
    setSteps(
      next.members.map((member) => ({
        characterId: member.characterId ?? `slot-${member.slotIndex + 1}`,
        characterName: member.characterId ? characterById.get(member.characterId)?.name : undefined,
        status: "queued",
        detail: "調査ジョブの開始を待っています。",
      })),
    );
    setNotice("分析入力を検証しました。Codex調査ジョブを開始できます。");
    setView("analysis");
  };

  const handleNewParty = () => {
    setDraft(createEmptyParty(createPartyId()));
    setNotice("新しい編成を開きました。");
    setView("party");
  };

  return (
    <div className="min-h-dvh bg-slate-950 text-slate-100">
      <a
        href="#workspace-main"
        className="sr-only z-50 rounded bg-amber-400 px-4 py-2 font-semibold text-slate-950 focus:not-sr-only focus:fixed focus:left-4 focus:top-4"
      >
        メインコンテンツへ
      </a>
      <div className="grid min-h-dvh lg:grid-cols-[17rem_minmax(0,1fr)_19rem]">
        <aside
          className="border-b border-slate-800 bg-slate-900/55 p-5 lg:border-b-0 lg:border-r"
          aria-label="編成ナビゲーション"
        >
          <div>
            <p className="text-xs font-semibold uppercase tracking-[0.18em] text-amber-400">Genshin Reco</p>
            <h1 className="mt-2 text-balance text-xl font-bold">根拠付き編成ビルド</h1>
            <p className="mt-2 text-sm leading-6 text-slate-400">
              Ver.{catalog.gameVersion} / {catalog.characters.length}キャラ
            </p>
          </div>

          <nav className="mt-6 grid gap-2 sm:grid-cols-3 lg:grid-cols-1" aria-label="主要画面">
            {NAVIGATION.map(({ id, label, icon: Icon }) => (
              <button
                key={id}
                type="button"
                className={cn(
                  "flex min-h-11 items-center gap-3 rounded-lg px-3 py-2 text-left font-semibold",
                  view === id
                    ? "bg-slate-800 text-amber-300"
                    : "text-slate-300 hover:bg-slate-900 hover:text-slate-100",
                )}
                onClick={() => setView(id)}
                aria-current={view === id ? "page" : undefined}
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
                filteredParties.map((party) => (
                  <button
                    key={party.partyId ?? party.id}
                    type="button"
                    className="w-full rounded-lg px-3 py-2 text-left text-sm text-slate-300 hover:bg-slate-800 hover:text-slate-100"
                    onClick={() => {
                      setDraft(party);
                      setView("party");
                    }}
                  >
                    {party.name}
                  </button>
                ))
              )}
            </div>
          </section>
        </aside>

        <main id="workspace-main" className="min-w-0 px-5 py-7 sm:px-8 lg:px-10 lg:py-9">
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
            <PartyBuilder
              catalog={catalog}
              draft={draft}
              onChange={setDraft}
              onSave={handleSave}
              onAnalyze={handleAnalyze}
            />
          ) : null}
          {view === "analysis" ? (
            <div className="space-y-9">
              <AnalysisProgressPanel
                status={analysisStatus}
                characterSteps={steps}
                lastResultValidity={null}
                onCancel={analysisStatus === "queued" ? () => setAnalysisStatus("cancelled") : undefined}
              />
              <TeamResultPanel resolution={null} validity="current" />
            </div>
          ) : null}
          {view === "settings" ? <Gate0Screen embedded /> : null}
        </main>

        <aside
          className="border-t border-slate-800 bg-slate-900/30 p-5 lg:border-l lg:border-t-0"
          aria-label="現在の編成情報"
        >
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
        </aside>
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
          message:
            error instanceof Error ? error.message : "カタログの検証に失敗しました。",
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
