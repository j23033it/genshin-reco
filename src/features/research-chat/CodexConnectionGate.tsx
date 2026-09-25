import { useCallback, useEffect, useState } from "react";
import { CodexDeviceLoginPanel } from "../gate0/CodexDeviceLoginPanel";
import { readCodexLoginStatus } from "../gate0/codexDeviceLogin";
import ResearchChatPrototype from "./ResearchChatPrototype";

type ConnectionState =
  | { status: "checking" }
  | { status: "ready" }
  | { status: "login" }
  | { status: "error"; message: string };

function errorText(error: unknown) {
  return error instanceof Error
    ? error.message
    : typeof error === "string"
      ? error
      : "Codexの接続状態を確認できませんでした。";
}

export function CodexConnectionGate() {
  const desktopRuntime = Boolean(window.__TAURI_INTERNALS__);
  const [state, setState] = useState<ConnectionState>({ status: "checking" });

  const recheck = useCallback(async () => {
    if (!desktopRuntime) return;
    setState({ status: "checking" });
    try {
      const result = await readCodexLoginStatus();
      setState({ status: result.authenticated ? "ready" : "login" });
    } catch (error) {
      setState({ status: "error", message: errorText(error) });
    }
  }, [desktopRuntime]);

  useEffect(() => {
    if (!desktopRuntime) return;
    const timer = window.setTimeout(() => void recheck(), 0);
    return () => window.clearTimeout(timer);
  }, [desktopRuntime, recheck]);

  if (desktopRuntime && state.status === "ready") {
    return <ResearchChatPrototype />;
  }

  return (
    <main className="grid min-h-dvh place-items-center bg-slate-950 px-5 py-10 text-slate-100">
      <section className="w-full max-w-xl rounded-2xl border border-slate-800 bg-slate-900 p-6 sm:p-8">
        <p className="text-sm font-semibold text-amber-300">編成ノート</p>
        <h1 className="mt-2 text-balance text-2xl font-bold">Codexに接続</h1>
        {!desktopRuntime ? (
          <p className="mt-4 text-pretty text-sm leading-7 text-slate-300">
            認証と実際の編成調査はデスクトップアプリで使えます。開発中は
            <code className="mx-1 rounded bg-slate-800 px-1.5 py-0.5">npm run tauri dev</code>
            で起動してください。
          </p>
        ) : state.status === "checking" ? (
          <p className="mt-4 text-sm text-slate-300" role="status">
            認証状態を確認しています…
          </p>
        ) : state.status === "login" ? (
          <>
            <p className="mt-4 text-pretty text-sm leading-7 text-slate-300">
              ChatGPTアカウントでログインすると、指定した4人の情報を調査できます。
            </p>
            <CodexDeviceLoginPanel onRecheck={() => void recheck()} />
          </>
        ) : state.status === "error" ? (
          <>
            <p className="mt-4 break-words text-sm leading-7 text-rose-200" role="alert">
              {state.message}
            </p>
            <button
              type="button"
              className="mt-5 min-h-11 rounded-xl bg-amber-300 px-4 py-2 font-semibold text-slate-950 hover:bg-amber-200"
              onClick={() => void recheck()}
            >
              接続を再確認
            </button>
          </>
        ) : null}
      </section>
    </main>
  );
}
