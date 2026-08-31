import { useCallback, useEffect, useRef, useState, type ReactNode } from "react";
import { cn } from "../../lib/cn";
import { CodexDeviceLoginPanel } from "./CodexDeviceLoginPanel";
import { CodexGate0SmokePanel } from "./CodexGate0SmokePanel";
import { probeCodexEnvironment } from "./probeCodexEnvironment";
import type { Gate0ProbeReport } from "./types";

type ProbeState =
  | { status: "idle" }
  | { status: "loading" }
  | { status: "success"; report: Gate0ProbeReport }
  | { status: "error"; message: string };

const valueOrFallback = (value: string | null) => value || "未取得";

const isAccountConnected = (report: Gate0ProbeReport) => Boolean(report.account?.authMode);

function CodexConnectionStatus({ state }: { state: ProbeState }) {
  const status =
    state.status === "loading"
      ? {
          label: "確認中",
          description: "Codex App Serverへ接続して、アカウントの連携状態を確認しています。",
          className: "border-slate-700 bg-slate-950/50 text-slate-200",
          badgeClassName: "border-slate-600 bg-slate-800 text-slate-200",
        }
      : state.status === "error"
        ? {
            label: "確認できません",
            description: "Codex環境へ接続できませんでした。表示されたエラーを確認して再実行してください。",
            className: "border-rose-400/50 bg-rose-400/10 text-rose-100",
            badgeClassName: "border-rose-300/50 bg-rose-300/10 text-rose-100",
          }
        : state.status === "success" && isAccountConnected(state.report)
          ? {
              label: "連携済み",
              description: "Codexの認証情報を確認しました。このアプリはCodexと連携済みです。",
              className: "border-emerald-400/50 bg-emerald-400/10 text-emerald-100",
              badgeClassName: "border-emerald-300/50 bg-emerald-300/10 text-emerald-100",
            }
          : state.status === "success"
            ? {
                label: "未連携",
                description: "Codexへログインすると、このアプリから推薦機能を利用できます。",
                className: "border-amber-400/50 bg-amber-400/10 text-amber-100",
                badgeClassName: "border-amber-300/50 bg-amber-300/10 text-amber-100",
              }
            : {
                label: "未確認",
                description: "この画面を開くと、Codexとの連携状態を自動で確認します。",
                className: "border-slate-700 bg-slate-950/50 text-slate-200",
                badgeClassName: "border-slate-600 bg-slate-800 text-slate-200",
              };

  return (
    <div
      className={cn("mt-6 rounded-xl border p-4", status.className)}
      role="status"
      aria-live="polite"
      aria-atomic="true"
      aria-busy={state.status === "loading"}
      data-testid="codex-connection-status"
    >
      <div className="flex flex-wrap items-center justify-between gap-3">
        <p className="font-semibold">Codex連携状態</p>
        <span className={cn("rounded-full border px-3 py-1 text-sm font-semibold", status.badgeClassName)}>
          {status.label}
        </span>
      </div>
      <p className="mt-2 text-pretty text-sm leading-6">{status.description}</p>
    </div>
  );
}

function BooleanValue({
  value,
  trueLabel,
  falseLabel,
}: {
  value: boolean;
  trueLabel: string;
  falseLabel: string;
}) {
  return <span className="font-semibold">{value ? trueLabel : falseLabel}</span>;
}

function ReportDetails({ report, onRecheck }: { report: Gate0ProbeReport; onRecheck: () => void }) {
  const accountConnected = isAccountConnected(report);
  const smokeAvailable = report.account?.authMode === "chatgpt";

  return (
    <div className="mt-8 space-y-6" data-testid="gate0-report">
      <section aria-labelledby="environment-heading">
        <div className="flex flex-wrap items-baseline justify-between gap-3">
          <h3 id="environment-heading" className="text-balance text-xl font-semibold text-slate-100">
            確認結果
          </h3>
          <span className="tabular-nums text-sm text-slate-400">Codex環境</span>
        </div>
        <dl className="mt-4 grid gap-3 sm:grid-cols-2">
          <ReportItem label="Codexの場所" value={valueOrFallback(report.codexPath)} wide />
          <ReportItem label="Codexバージョン" value={valueOrFallback(report.codexVersion)} />
          <ReportItem
            label="バージョン要件"
            value={<BooleanValue value={report.versionSupported} trueLabel="対応" falseLabel="未対応" />}
          />
          <ReportItem
            label="App Server"
            value={<BooleanValue value={report.appServerInitialized} trueLabel="初期化済み" falseLabel="未初期化" />}
          />
          <ReportItem label="分離ホーム" value={valueOrFallback(report.isolatedHome)} wide />
          <ReportItem label="プラットフォーム" value={valueOrFallback(report.platformFamily)} />
          <ReportItem label="OS" value={valueOrFallback(report.platformOs)} />
          <ReportItem
            label="利用上限情報"
            value={
              <BooleanValue
                value={report.rateLimitsAvailable}
                trueLabel="取得済み"
                falseLabel={accountConnected ? "取得できません（連携済み）" : "連携後に取得"}
              />
            }
          />
        </dl>
      </section>

      <section aria-labelledby="account-heading">
        <h3 id="account-heading" className="text-balance text-xl font-semibold text-slate-100">
          アカウント
        </h3>
        {!accountConnected ? (
          <CodexDeviceLoginPanel onRecheck={onRecheck} />
        ) : (
          <>
            <dl className="mt-4 grid gap-3 sm:grid-cols-2">
              <ReportItem label="認証方式" value={valueOrFallback(report.account?.authMode ?? null)} />
              <ReportItem label="プラン" value={valueOrFallback(report.account?.planType ?? null)} />
            </dl>
            {!report.rateLimitsAvailable && (
              <p className="mt-4 rounded-lg border border-amber-300/40 bg-amber-300/5 p-4 text-pretty text-sm leading-6 text-amber-100">
                Codex連携は有効です。利用上限情報だけを現在表示できませんが、認証状態には影響しません。
              </p>
            )}
            {smokeAvailable ? (
              <CodexGate0SmokePanel />
            ) : (
              <p className="mt-4 rounded-lg border border-amber-300/40 bg-amber-300/5 p-4 text-pretty text-sm leading-6 text-amber-100">
                実ターンのスモークテストにはChatGPT認証が必要です。現在の認証方式では実行できません。
              </p>
            )}
          </>
        )}
      </section>

      {report.diagnostics.length > 0 && (
        <section aria-labelledby="diagnostics-heading">
          <h3 id="diagnostics-heading" className="text-balance text-xl font-semibold text-slate-100">
            診断メモ
          </h3>
          <ul className="mt-3 list-disc space-y-2 pl-5 text-pretty text-sm leading-6 text-slate-400">
            {report.diagnostics.map((diagnostic, index) => (
              <li key={`${diagnostic}-${index}`}>{diagnostic}</li>
            ))}
          </ul>
        </section>
      )}
    </div>
  );
}

function ReportItem({
  label,
  value,
  wide = false,
}: {
  label: string;
  value: ReactNode;
  wide?: boolean;
}) {
  return (
    <div className={cn("min-w-0 rounded-lg border border-slate-800 bg-slate-900/70 p-4", wide && "sm:col-span-2")}>
      <dt className="text-pretty text-sm text-slate-400">{label}</dt>
      <dd className="mt-2 break-words text-pretty tabular-nums text-slate-100">{value}</dd>
    </div>
  );
}

function LoadingReport() {
  return (
    <div className="mt-8 space-y-4" data-testid="gate0-loading" aria-hidden="true">
      <div className="h-7 w-32 rounded bg-slate-800" />
      <div className="grid gap-3 sm:grid-cols-2">
        {["a", "b", "c", "d"].map((key) => (
          <div key={key} className="h-20 rounded-lg border border-slate-800 bg-slate-900" />
        ))}
      </div>
    </div>
  );
}

export function Gate0Screen({ embedded = false }: { embedded?: boolean }) {
  const desktopRuntime = Boolean(window.__TAURI_INTERNALS__);
  const [probeState, setProbeState] = useState<ProbeState>({ status: "idle" });
  const probingRef = useRef(false);

  const handleProbe = useCallback(async () => {
    if (probingRef.current) return;
    probingRef.current = true;
    setProbeState({ status: "loading" });

    try {
      const report = await probeCodexEnvironment();
      setProbeState({ status: "success", report });
    } catch (error) {
      const message =
        error instanceof Error
          ? error.message
          : typeof error === "string"
            ? error
            : "Codex環境の確認に失敗しました。もう一度お試しください。";
      setProbeState({ status: "error", message });
    } finally {
      probingRef.current = false;
    }
  }, []);

  useEffect(() => {
    if (!desktopRuntime) return;
    const timerId = window.setTimeout(() => {
      void handleProbe();
    }, 0);
    return () => window.clearTimeout(timerId);
  }, [desktopRuntime, handleProbe]);

  const isLoading = probeState.status === "loading";

  const Root = embedded ? "div" : "main";

  return (
    <Root
      className={cn(
        embedded
          ? "text-slate-100"
          : "min-h-dvh bg-slate-950 px-6 py-10 text-slate-100 sm:px-10 sm:py-14",
      )}
    >
      <section className="mx-auto max-w-4xl">
        <header>
          <p className="text-sm font-semibold text-amber-400">原神 Ver.7.0 対応</p>
          <h1 className="mt-3 max-w-3xl text-balance text-4xl font-bold leading-tight sm:text-5xl">
            根拠付き・編成連動ビルド推薦
          </h1>
          <p className="mt-5 max-w-2xl text-pretty text-lg leading-8 text-slate-300">
            4人編成、武器、精錬、命ノ星座と検証済みの根拠をもとに、条件へ適合する聖遺物候補を比較します。
          </p>
        </header>

        <section className="mt-12 rounded-2xl border border-slate-800 bg-slate-900/40 p-6 shadow-sm sm:p-8" aria-labelledby="gate0-heading">
          <p className="text-sm font-semibold text-amber-400">Gate 0</p>
          <h2 id="gate0-heading" className="mt-2 text-balance text-2xl font-semibold sm:text-3xl">
            Codexの環境を確認
          </h2>
          <p className="mt-3 max-w-2xl text-pretty leading-7 text-slate-300">
            推薦を始める前に、Codex App Serverへ接続できるか確認します。確認結果はこの画面でのみ表示します。
          </p>

          <CodexConnectionStatus state={probeState} />

          <div className="mt-6">
            <button
              type="button"
              className="inline-flex min-h-11 items-center justify-center rounded-lg bg-amber-400 px-5 py-3 font-semibold text-slate-950 shadow-sm hover:bg-amber-300 focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-amber-400 disabled:cursor-not-allowed disabled:opacity-60"
              onClick={handleProbe}
              disabled={isLoading || !desktopRuntime}
              aria-busy={isLoading}
            >
              {isLoading ? "確認中…" : probeState.status === "idle" ? "環境を確認" : "再確認"}
            </button>
            {probeState.status === "error" && (
              <p className="mt-3 text-pretty text-sm leading-6 text-amber-200" role="alert">
                {probeState.message}
              </p>
            )}
            {!desktopRuntime ? (
              <p className="mt-3 text-pretty text-sm leading-6 text-slate-400">
                Codex連携の確認はデスクトップアプリで利用できます。
              </p>
            ) : null}
          </div>

          {isLoading && <LoadingReport />}
          {probeState.status === "success" && <ReportDetails report={probeState.report} onRecheck={handleProbe} />}
        </section>
      </section>
    </Root>
  );
}
