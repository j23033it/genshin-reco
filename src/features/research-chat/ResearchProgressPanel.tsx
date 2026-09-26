import { Check, LoaderCircle } from "lucide-react";
import { OperationProgress } from "../../components/OperationProgress";
import { cn } from "../../lib/cn";
import type { ResearchProgress } from "./types";

const STEPS = ["準備", "調査", "検証・保存"];

export function ResearchProgressPanel({ progress, cancelling }: {
  progress: ResearchProgress | null;
  cancelling: boolean;
}) {
  const completed = progress?.stage === "completed";
  const step = completed || progress?.stage === "validating" ? 2 : progress ? 1 : 0;
  const label = cancelling ? "調査をキャンセル中…"
    : completed ? "調査結果を保存しました"
      : ["調査を開始しています…", "根拠ページを調査中", "結果を検証・保存中"][step];

  return (
    <>
      <p className="text-sm font-semibold text-amber-300">Codexが調査中</p>
      <h1 className="mt-2 text-balance text-2xl font-bold">4人の情報を集めています</h1>
      <ol aria-label="調査の段階" className="mt-6 grid grid-cols-3 gap-2">
        {STEPS.map((name, index) => (
          <li
            key={name}
            aria-current={!completed && index === step ? "step" : undefined}
            className={cn(
              "min-w-0 border-t-4 pt-3 text-xs sm:text-sm",
              completed || index < step ? "border-emerald-400 text-emerald-200"
                : index === step ? "border-amber-300 text-amber-200" : "border-slate-700 text-slate-400",
            )}
          >
            <span className="flex flex-col items-start gap-2">
              {completed || index < step ? <Check aria-hidden="true" size={16} />
                : index === step ? <LoaderCircle aria-hidden="true" className="size-4 motion-safe:animate-spin" />
                  : <span aria-hidden="true" className="tabular-nums">{index + 1}.</span>}
              <span>{name}</span>
            </span>
          </li>
        ))}
      </ol>
      <OperationProgress
        className="mt-6"
        label={label}
        detail={progress?.detail ? `${progress.memberName ? `${progress.memberName}：` : ""}${progress.detail}` : undefined}
        value={completed && !cancelling ? 100 : undefined}
      />
    </>
  );
}
