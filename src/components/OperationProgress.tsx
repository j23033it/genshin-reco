import { LoaderCircle } from "lucide-react";
import { cn } from "../lib/cn";

export function OperationProgress({
  label,
  detail,
  value,
  className,
}: {
  label: string;
  detail?: string;
  value?: number;
  className?: string;
}) {
  const percent = value !== undefined && Number.isFinite(value)
    ? Math.min(100, Math.max(0, value))
    : undefined;

  return (
    <div className={cn("min-w-0 text-sm text-amber-200", className)}>
      <div className="flex items-start gap-3">
        {percent === undefined ? (
          <LoaderCircle aria-hidden="true" className="mt-0.5 size-5 shrink-0 motion-safe:animate-spin" />
        ) : null}
        <div className="min-w-0 flex-1" role="status" aria-live="polite" aria-atomic="true">
          <p className="break-words font-medium">{label}</p>
          {detail ? <p className="mt-2 whitespace-pre-wrap break-words leading-7 text-slate-300">{detail}</p> : null}
        </div>
        {percent !== undefined ? (
          <span aria-hidden="true" className="shrink-0 tabular-nums">{Math.floor(percent)}%</span>
        ) : null}
      </div>
      <div
        role="progressbar"
        aria-label={label}
        aria-valuemin={0}
        aria-valuemax={100}
        aria-valuenow={percent}
        aria-valuetext={percent === undefined ? "処理中" : `${Math.floor(percent)}%`}
        className={percent === undefined ? "sr-only" : "mt-2 h-2 overflow-hidden rounded-full bg-slate-700"}
      >
        {percent !== undefined ? (
          <div
            className="h-full origin-left rounded-full bg-amber-300 motion-safe:transition-transform"
            style={{ transform: `scaleX(${percent / 100})` }}
          />
        ) : null}
      </div>
    </div>
  );
}
