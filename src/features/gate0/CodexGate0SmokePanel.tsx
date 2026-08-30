import { useState } from "react";
import { runCodexGate0Smoke } from "./codexGate0Smoke";
import type { Gate0SmokeReport } from "./types";

type SmokeState =
  | { status: "idle" }
  | { status: "running" }
  | { status: "success"; report: Gate0SmokeReport }
  | { status: "error"; message: string };

const errorMessage = (error: unknown) =>
  error instanceof Error
    ? error.message
    : typeof error === "string"
      ? error
      : "Gate 0スモークの実行に失敗しました。";

function Result({ label, passed }: { label: string; passed: boolean }) {
  return (
    <div className="rounded-lg border border-slate-700 bg-slate-950/40 p-3">
      <dt className="text-sm text-slate-400">{label}</dt>
      <dd className={passed ? "mt-1 font-semibold text-emerald-300" : "mt-1 font-semibold text-rose-300"}>
        {passed ? "合格" : "不合格"}
      </dd>
    </div>
  );
}

export function CodexGate0SmokePanel() {
  const [state, setState] = useState<SmokeState>({ status: "idle" });

  const handleRun = async () => {
    if (state.status === "running") return;
    setState({ status: "running" });
    try {
      setState({ status: "success", report: await runCodexGate0Smoke() });
    } catch (error) {
      setState({ status: "error", message: errorMessage(error) });
    }
  };

  const report = state.status === "success" ? state.report : null;
  const passed =
    report?.structuredOutputValid === true &&
    report.webSearchObserved === true &&
    report.cancellationObserved === true;
  const rerouteLabel = report?.modelRerouted
    ? (report.reroutedFrom ?? "不明") + " → " + (report.reroutedTo ?? "不明")
    : "なし";

  return (
    <div className="mt-4 rounded-lg border border-slate-700 bg-slate-900/70 p-4">
      <h4 className="text-lg font-semibold text-slate-100">実ターンのスモークテスト</h4>
      <p className="mt-2 text-pretty text-sm leading-6 text-slate-300">
        構造化出力、Web検索イベント、中断を実際のCodexターンで確認します。少量の利用枠を消費します。
      </p>
      <button
        type="button"
        className="mt-4 inline-flex min-h-11 items-center justify-center rounded-lg border border-amber-300/60 px-4 py-2 font-semibold text-amber-100 hover:bg-amber-300/10 disabled:cursor-not-allowed disabled:opacity-60"
        onClick={handleRun}
        disabled={state.status === "running"}
        aria-busy={state.status === "running"}
      >
        {state.status === "running" ? "スモーク実行中…" : "Gate 0スモークを実行"}
      </button>

      {state.status === "error" && (
        <p className="mt-3 text-pretty text-sm leading-6 text-rose-300" role="alert">
          {state.message}
        </p>
      )}

      {report && (
        <div className="mt-4">
          <p className={passed ? "font-semibold text-emerald-300" : "font-semibold text-rose-300"}>
            {passed ? "Gate 0スモークは合格です。" : "Gate 0スモークに不合格項目があります。"}
          </p>
          <dl className="mt-3 grid gap-3 sm:grid-cols-3">
            <Result label="構造化出力" passed={report.structuredOutputValid} />
            <Result label="Web検索イベント" passed={report.webSearchObserved} />
            <Result label="ターン中断" passed={report.cancellationObserved} />
          </dl>
          <p className="mt-3 text-pretty text-sm leading-6 text-slate-400">
            指示元一覧: {report.instructionSourcesSupported ? "検証済み" : "Codex 0.118互換モード"}
            ／モデルreroute: {rerouteLabel}
          </p>
        </div>
      )}

      {state.status !== "idle" && (
        <p className="sr-only" role="status" aria-live="polite">
          {state.status === "running"
            ? "Gate 0スモークを実行しています。"
            : state.status === "error"
              ? "Gate 0スモークに失敗しました。"
              : passed
                ? "Gate 0スモークに合格しました。"
                : "Gate 0スモークに不合格項目があります。"}
        </p>
      )}
    </div>
  );
}
