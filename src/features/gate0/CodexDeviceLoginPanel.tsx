import { useEffect, useState } from "react";
import { cn } from "../../lib/cn";
import {
  cancelCodexDeviceLogin,
  readCodexLoginStatus,
  startCodexDeviceLogin,
} from "./codexDeviceLogin";
import type { CodexDeviceLoginChallenge } from "./types";

const LOGIN_POLL_INTERVAL_MS = 2000;

type LoginState =
  | { status: "idle" }
  | { status: "starting" }
  | { status: "active"; challenge: CodexDeviceLoginChallenge }
  | { status: "cancelling"; challenge: CodexDeviceLoginChallenge }
  | { status: "success" }
  | { status: "error"; message: string };

type CopyState = "idle" | "copying" | "copied" | "failed";

function errorMessage(error: unknown, fallback: string) {
  if (error instanceof Error && error.message) return error.message;
  if (typeof error === "string" && error) return error;
  return fallback;
}

export function CodexDeviceLoginPanel({ onRecheck }: { onRecheck: () => void }) {
  const [loginState, setLoginState] = useState<LoginState>({ status: "idle" });
  const [copyState, setCopyState] = useState<CopyState>("idle");
  const activeLoginId = loginState.status === "active" ? loginState.challenge.loginId : null;

  useEffect(() => {
    if (activeLoginId === null) return;

    let disposed = false;
    let timerId: number | undefined;

    const pollLoginStatus = async () => {
      try {
        const result = await readCodexLoginStatus();
        if (disposed) return;

        if (result.authenticated) {
          setLoginState({ status: "success" });
          onRecheck();
          return;
        }

        if (result.loginCompleted === true) {
          setLoginState({ status: "success" });
          return;
        }

        if (result.loginError || result.loginCompleted === false) {
          setLoginState({
            status: "error",
            message: result.loginError || "ログインが完了しませんでした。もう一度お試しください。",
          });
          return;
        }

        timerId = window.setTimeout(pollLoginStatus, LOGIN_POLL_INTERVAL_MS);
      } catch (error) {
        if (!disposed) {
          setLoginState({
            status: "error",
            message: errorMessage(error, "ログイン状態の確認に失敗しました。もう一度お試しください。"),
          });
        }
      }
    };

    timerId = window.setTimeout(pollLoginStatus, LOGIN_POLL_INTERVAL_MS);
    return () => {
      disposed = true;
      if (timerId !== undefined) window.clearTimeout(timerId);
    };
  }, [activeLoginId, onRecheck]);

  const handleStart = async () => {
    if (loginState.status === "starting" || loginState.status === "active" || loginState.status === "cancelling") return;
    setCopyState("idle");
    setLoginState({ status: "starting" });

    try {
      const challenge = await startCodexDeviceLogin();
      setLoginState({ status: "active", challenge });
    } catch (error) {
      setLoginState({
        status: "error",
        message: errorMessage(error, "ログインの開始に失敗しました。もう一度お試しください。"),
      });
    }
  };

  const handleCancel = async () => {
    if (loginState.status !== "active") return;
    const { challenge } = loginState;
    setLoginState({ status: "cancelling", challenge });

    try {
      await cancelCodexDeviceLogin(challenge.loginId);
      setCopyState("idle");
      setLoginState({ status: "idle" });
    } catch (error) {
      setLoginState({
        status: "error",
        message: errorMessage(error, "ログインの取消に失敗しました。もう一度お試しください。"),
      });
    }
  };

  const handleCopy = async () => {
    if (loginState.status !== "active" || copyState === "copying") return;
    setCopyState("copying");

    try {
      if (!navigator.clipboard?.writeText) throw new Error("この環境ではクリップボードを利用できません。");
      await navigator.clipboard.writeText(loginState.challenge.userCode);
      setCopyState("copied");
    } catch {
      setCopyState("failed");
    }
  };

  const isBusy = loginState.status === "starting" || loginState.status === "cancelling";

  return (
    <div className="mt-3 rounded-lg border border-amber-400/50 bg-amber-400/10 p-4" data-testid="codex-login-panel">
      <h4 className="text-balance text-lg font-semibold text-amber-100">Codexへログインしてください</h4>

      {loginState.status === "idle" || loginState.status === "error" ? (
        <>
          <p className="mt-2 text-pretty text-sm leading-6 text-amber-200">
            デバイスコードを使ってログインできます。開始後に表示されるURLをブラウザで開いてください。
          </p>
          <button
            type="button"
            className="mt-4 inline-flex min-h-11 items-center justify-center rounded-lg bg-amber-400 px-5 py-3 font-semibold text-slate-950 shadow-sm hover:bg-amber-300 disabled:cursor-not-allowed disabled:opacity-60"
            onClick={handleStart}
            disabled={isBusy}
          >
            ログインを開始
          </button>
          {loginState.status === "error" && (
            <p className="mt-3 text-pretty text-sm leading-6 text-amber-200" role="alert">
              {loginState.message}
            </p>
          )}
        </>
      ) : null}

      {loginState.status === "starting" && (
        <p className="mt-3 text-pretty text-sm leading-6 text-amber-200" role="status" aria-live="polite">
          ログイン情報を準備しています…
        </p>
      )}

      {(loginState.status === "active" || loginState.status === "cancelling") && (
        <div className="mt-4 space-y-4">
          <p className="text-pretty text-sm leading-6 text-amber-200">
            表示されたURLをブラウザで開き、次のコードを入力してください。このアプリはブラウザを自動で開きません。
          </p>
          <dl className="grid gap-3 sm:grid-cols-2">
            <div className="min-w-0 rounded-lg border border-amber-300/30 bg-slate-950/50 p-3 sm:col-span-2">
              <dt className="text-sm text-amber-200/80">認証URL</dt>
              <dd className="mt-2 break-all font-mono text-sm text-slate-100">{loginState.challenge.verificationUrl}</dd>
            </div>
            <div className="min-w-0 rounded-lg border border-amber-300/30 bg-slate-950/50 p-3">
              <dt className="text-sm text-amber-200/80">ユーザーコード</dt>
              <dd className="mt-2 break-all font-mono text-lg font-semibold tabular-nums text-slate-100">
                {loginState.challenge.userCode}
              </dd>
            </div>
          </dl>
          <div className="flex flex-wrap items-center gap-3">
            <button
              type="button"
              className="inline-flex min-h-11 items-center justify-center rounded-lg border border-amber-300/60 px-4 py-2 font-semibold text-amber-100 hover:bg-amber-300/10 disabled:cursor-not-allowed disabled:opacity-60"
              onClick={handleCopy}
              disabled={loginState.status === "cancelling" || copyState === "copying"}
            >
              {copyState === "copying" ? "コピー中…" : "コードをコピー"}
            </button>
            <button
              type="button"
              className="inline-flex min-h-11 items-center justify-center rounded-lg border border-slate-600 px-4 py-2 font-semibold text-slate-200 hover:bg-slate-800 disabled:cursor-not-allowed disabled:opacity-60"
              onClick={handleCancel}
              disabled={loginState.status === "cancelling"}
            >
              {loginState.status === "cancelling" ? "取消中…" : "ログインを取消"}
            </button>
          </div>
          <p className={cn("text-pretty text-sm leading-6", copyState === "failed" ? "text-amber-200" : "text-slate-400")} role="status" aria-live="polite">
            {copyState === "copied"
              ? "コードをクリップボードへコピーしました。"
              : copyState === "failed"
                ? "コードをコピーできませんでした。表示されたコードを手入力してください。"
                : loginState.status === "cancelling"
                  ? "ログインを取消しています…"
                  : "ログイン状態を確認しています。"
            }
          </p>
        </div>
      )}

      {loginState.status === "success" && (
        <div className="mt-3" role="status" aria-live="polite">
          <p className="text-pretty leading-7 text-emerald-200">ログインが完了しました。環境を再確認して続行してください。</p>
          <button
            type="button"
            className="mt-4 inline-flex min-h-11 items-center justify-center rounded-lg bg-amber-400 px-5 py-3 font-semibold text-slate-950 shadow-sm hover:bg-amber-300"
            onClick={onRecheck}
          >
            環境を再確認
          </button>
        </div>
      )}
    </div>
  );
}
