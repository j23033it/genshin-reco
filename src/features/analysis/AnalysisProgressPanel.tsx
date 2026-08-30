import { cn } from "../../lib/cn";
import type { AnalysisStatus, ResultValidity } from "../../domain/analysisTypes";
import type {
  AnalysisCharacterStepStatus,
  AnalysisProgressPanelProps,
  CharacterAnalysisStep,
} from "./types";

const ANALYSIS_STATUS_LABELS: Record<AnalysisStatus, string> = {
  queued: "分析待ち",
  starting_codex: "分析を開始しています",
  researching: "根拠を調査しています",
  verifying_sources: "根拠を検証しています",
  reconciling: "候補を照合しています",
  solving: "編成を解いています",
  persisting: "結果を保存しています",
  succeeded: "分析が完了しました",
  failed: "分析に失敗しました",
  cancelled: "分析をキャンセルしました",
  superseded: "新しい分析に置き換えられました",
  abandoned: "分析を終了しました",
};

const CHARACTER_STEP_LABELS: Record<AnalysisCharacterStepStatus, string> = {
  queued: "待機中",
  researching: "調査中",
  verifying: "検証中",
  reconciling: "照合中",
  solving: "解決中",
  completed: "完了",
  failed: "失敗",
  cancelled: "キャンセル済み",
};

const VALIDITY_LABELS: Record<Exclude<ResultValidity, "current">, string> = {
  soft_stale: "以前の結果（要再確認）",
  hard_stale: "古い結果（再分析が必要）",
  invalid: "無効な結果",
};

const ACTIVE_ANALYSIS_STATUSES = new Set<AnalysisStatus>([
  "queued",
  "starting_codex",
  "researching",
  "verifying_sources",
  "reconciling",
  "solving",
  "persisting",
]);

function getCharacterStepStatus(step: CharacterAnalysisStep): AnalysisCharacterStepStatus {
  return "status" in step ? step.status : step.stage;
}

function getCharacterLabel(step: CharacterAnalysisStep) {
  return step.characterName ?? step.name ?? step.characterId;
}

function isStaleValidity(validity: ResultValidity | null): validity is Exclude<ResultValidity, "current"> {
  return validity !== null && validity !== "current";
}

function ProgressStep({ step }: { step: CharacterAnalysisStep }) {
  const stepStatus = getCharacterStepStatus(step);
  const stepLabel = CHARACTER_STEP_LABELS[stepStatus];

  return (
    <li
      className="min-w-0 rounded-lg border border-slate-700 bg-slate-950/50 p-4"
      data-character-id={step.characterId}
      data-step-status={stepStatus}
    >
      <div className="flex min-w-0 flex-wrap items-start justify-between gap-2">
        <h3 className="min-w-0 break-words text-base font-semibold text-slate-100">
          {getCharacterLabel(step)}
        </h3>
        <span className="shrink-0 rounded-full border border-amber-300/50 px-2 py-1 text-sm font-semibold text-amber-200">
          {stepLabel}
        </span>
      </div>
      {step.detail ? <p className="mt-2 break-words text-pretty text-sm leading-6 text-slate-300">{step.detail}</p> : null}
      {step.error ? (
        <p className="mt-2 break-words text-pretty text-sm leading-6 text-rose-300" aria-live="polite">
          {step.error}
        </p>
      ) : null}
    </li>
  );
}

function StaleResultBanner({ validity }: { validity: Exclude<ResultValidity, "current"> }) {
  return (
    <aside
      className="rounded-lg border border-amber-300/60 bg-amber-400/10 p-4"
      data-testid="analysis-stale-banner"
      aria-label="以前の分析結果"
    >
      <p className="font-semibold text-amber-100">以前の分析結果を表示中</p>
      <p className="mt-1 break-words text-pretty text-sm leading-6 text-amber-200">
        {VALIDITY_LABELS[validity]}。分析が完了するまで、この結果を最新の判断には使わないでください。
      </p>
    </aside>
  );
}

function RecoveryMessage() {
  return (
    <p className="mt-2 break-words text-pretty text-sm leading-6 text-rose-200">
      入力内容と接続状態を確認してから、もう一度分析を実行してください。
    </p>
  );
}

function ProgressStatusMessage({ status }: { status: AnalysisStatus }) {
  return (
    <div className="rounded-lg border border-slate-700 bg-slate-900/70 p-4" role="status" aria-live="polite" aria-atomic="true">
      <p className="text-pretty font-semibold text-slate-100">{ANALYSIS_STATUS_LABELS[status]}</p>
      <p className="mt-1 text-pretty text-sm leading-6 text-slate-300">
        キャラクターごとの工程を表示しています。完了した工程と未完了の工程を確認できます。
      </p>
    </div>
  );
}

export function AnalysisProgressPanel({
  status,
  characterSteps,
  lastResultValidity,
  onCancel,
  error,
}: AnalysisProgressPanelProps) {
  const isActive = ACTIVE_ANALYSIS_STATUSES.has(status);
  const staleValidity = isStaleValidity(lastResultValidity) ? lastResultValidity : null;
  const visibleError = error ?? (status === "failed" ? "分析を完了できませんでした。" : null);

  return (
    <section className="space-y-4" aria-labelledby="analysis-progress-heading" data-testid="analysis-progress-panel">
      <div>
        <h2 id="analysis-progress-heading" className="text-balance text-xl font-bold text-slate-100">
          分析の進行状況
        </h2>
        <p className="mt-1 text-pretty text-sm leading-6 text-slate-300">
          進捗率ではなく、キャラクターごとの現在の工程を確認できます。
        </p>
      </div>

      <ProgressStatusMessage status={status} />

      {staleValidity ? <StaleResultBanner validity={staleValidity} /> : null}

      {isActive && onCancel ? (
        <button
          type="button"
          className="inline-flex min-h-11 items-center justify-center rounded-lg border border-slate-500 px-4 py-2 font-semibold text-slate-100 hover:bg-slate-800 focus-visible:outline-2 focus-visible:outline-amber-400 disabled:cursor-not-allowed disabled:opacity-60"
          onClick={onCancel}
          aria-label="分析をキャンセル"
        >
          分析をキャンセル
        </button>
      ) : null}

      {visibleError ? (
        <div className="rounded-lg border border-rose-400/60 bg-rose-400/10 p-4" role="alert">
          <p className="break-words text-pretty font-semibold text-rose-100">{visibleError}</p>
          <RecoveryMessage />
        </div>
      ) : null}

      {status === "cancelled" ? (
        <p className="rounded-lg border border-slate-700 bg-slate-900/60 p-4 text-pretty text-sm leading-6 text-slate-300">
          分析はキャンセルされました。必要であれば、編成を確認してから再実行してください。
        </p>
      ) : null}

      {characterSteps.length > 0 ? (
        <ol className={cn("grid gap-3", characterSteps.length > 1 ? "sm:grid-cols-2" : "grid-cols-1")}>
          {characterSteps.map((step) => (
            <ProgressStep key={step.characterId} step={step} />
          ))}
        </ol>
      ) : (
        <p className="rounded-lg border border-slate-700 bg-slate-900/60 p-4 text-pretty text-sm leading-6 text-slate-300">
          キャラクターごとの分析工程を準備しています。
        </p>
      )}
    </section>
  );
}

export type { AnalysisProgressPanelProps, AnalysisProgress, AnalysisCharacterStepStatus, CharacterAnalysisStep } from "./types";
