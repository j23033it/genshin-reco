import type { AnalysisStatus } from "../../domain/analysisTypes";
import { cn } from "../../lib/cn";
import type {
  AnalysisCharacterStepStatus,
  AnalysisProgressPanelProps,
  CharacterAnalysisStep,
} from "./types";

const ANALYSIS_STATUS_LABELS: Record<AnalysisStatus, string> = {
  queued: "分析の開始待ち",
  starting_codex: "分析を開始中",
  researching: "調査中",
  verifying_sources: "情報を検証中",
  reconciling: "候補を照合中",
  solving: "目標値を算出中",
  persisting: "結果を保存中",
  succeeded: "分析完了",
  failed: "分析失敗",
  cancelled: "分析中止",
  superseded: "分析を更新済み",
  abandoned: "分析終了",
};

const CHARACTER_STEP_LABELS: Record<AnalysisCharacterStepStatus, string> = {
  queued: "待機中",
  researching: "調査中",
  verifying: "検証中",
  reconciling: "照合中",
  solving: "算出中",
  completed: "調査完了",
  failed: "失敗",
  cancelled: "中止",
};

function getCharacterStepStatus(step: CharacterAnalysisStep): AnalysisCharacterStepStatus {
  return "status" in step ? step.status : step.stage;
}

function getCharacterLabel(step: CharacterAnalysisStep) {
  return step.characterName ?? step.name ?? step.characterId;
}

function ProgressStep({ step }: { step: CharacterAnalysisStep }) {
  const stepStatus = getCharacterStepStatus(step);
  const label = getCharacterLabel(step);
  const isFailure = stepStatus === "failed" || stepStatus === "cancelled";

  return (
    <li
      className="flex min-w-0 items-center gap-3 rounded-lg border border-slate-700 bg-slate-950/60 p-2.5"
      data-character-id={step.characterId}
      data-step-status={stepStatus}
    >
      {step.characterImageUrl ? (
        <img
          src={step.characterImageUrl}
          alt={`${label}のアイコン`}
          className="h-10 w-10 shrink-0 rounded-full border border-slate-600 bg-slate-900 object-cover"
        />
      ) : (
        <span
          className="grid h-10 w-10 shrink-0 place-items-center rounded-full border border-slate-600 bg-slate-900 font-semibold text-slate-300"
          aria-hidden="true"
        >
          {label.slice(0, 1)}
        </span>
      )}
      <div className="min-w-0 flex-1">
        <p className="truncate text-sm font-semibold text-slate-100">{label}</p>
        <p
          className={cn(
            "mt-0.5 text-xs font-semibold",
            isFailure ? "text-rose-300" : stepStatus === "completed" ? "text-emerald-300" : "text-amber-300",
          )}
        >
          {CHARACTER_STEP_LABELS[stepStatus]}
        </p>
      </div>
    </li>
  );
}

export function AnalysisProgressPanel({ status, characterSteps, onCancel }: AnalysisProgressPanelProps) {
  return (
    <section
      className="rounded-xl border border-amber-300/30 bg-amber-300/5 p-4"
      aria-labelledby="analysis-progress-heading"
      aria-busy="true"
      data-testid="analysis-progress-panel"
    >
      <div className="flex flex-wrap items-center justify-between gap-3">
        <div>
          <h2 id="analysis-progress-heading" className="text-balance font-bold text-slate-100">
            この編成を分析中
          </h2>
          <p className="mt-1 text-sm text-slate-300" role="status" aria-live="polite" aria-atomic="true">
            {ANALYSIS_STATUS_LABELS[status]}
          </p>
        </div>
        {onCancel ? (
          <button
            type="button"
            className="inline-flex min-h-11 items-center justify-center rounded-lg border border-slate-600 px-3 py-2 text-sm font-semibold text-slate-200 hover:bg-slate-800"
            onClick={onCancel}
          >
            キャンセル
          </button>
        ) : null}
      </div>
      <ol className="mt-3 grid gap-2 sm:grid-cols-2 xl:grid-cols-4">
        {characterSteps.map((step) => (
          <ProgressStep key={step.characterId} step={step} />
        ))}
      </ol>
    </section>
  );
}

export type { AnalysisProgressPanelProps, AnalysisProgress, AnalysisCharacterStepStatus, CharacterAnalysisStep } from "./types";
