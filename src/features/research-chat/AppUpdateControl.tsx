import { useState } from "react";
import { check, type Update } from "@tauri-apps/plugin-updater";
import { relaunch } from "@tauri-apps/plugin-process";
import { OperationProgress } from "../../components/OperationProgress";

type UpdateState =
  | { kind: "idle" }
  | { kind: "checking" }
  | { kind: "latest" }
  | { kind: "available"; update: Update }
  | { kind: "installing"; message: string; progress?: number }
  | { kind: "error"; message: string };

function errorMessage(error: unknown) {
  return error instanceof Error
    ? error.message
    : typeof error === "string"
      ? error
      : "更新を確認できませんでした。";
}

export function AppUpdateControl({ disabled = false }: { disabled?: boolean }) {
  const [state, setState] = useState<UpdateState>({ kind: "idle" });

  if (!window.__TAURI_INTERNALS__ || import.meta.env.DEV) return null;

  async function checkForUpdate() {
    setState({ kind: "checking" });
    try {
      const update = await check();
      setState(update ? { kind: "available", update } : { kind: "latest" });
    } catch (error) {
      setState({ kind: "error", message: errorMessage(error) });
    }
  }

  async function installUpdate(update: Update) {
    setState({ kind: "installing", message: "更新ファイルを取得中…" });
    let downloaded = 0;
    let total: number | undefined;
    try {
      await update.downloadAndInstall((event) => {
        if (event.event === "Started") {
          downloaded = 0;
          total = event.data.contentLength;
          setState({ kind: "installing", message: "更新ファイルを取得中…", progress: total && total > 0 ? 0 : undefined });
        } else if (event.event === "Progress") {
          downloaded += event.data.chunkLength;
          setState({ kind: "installing", message: "更新ファイルを取得中…", progress: total && total > 0 ? downloaded * 100 / total : undefined });
        } else if (event.event === "Finished") {
          setState({ kind: "installing", message: "更新を適用中…" });
        }
      });
      setState({ kind: "installing", message: "再起動中…" });
      await relaunch();
    } catch (error) {
      setState({ kind: "error", message: errorMessage(error) });
    }
  }

  const busy = state.kind === "checking" || state.kind === "installing";
  return (
    <div className="flex flex-wrap items-center justify-end gap-x-3 gap-y-1 text-xs">
      {state.kind === "available" ? (
        <>
          <span role="status" className="text-amber-200">
            新しい版 {state.update.version}
          </span>
          <button
            type="button"
            disabled={disabled}
            className="min-h-10 rounded-lg border border-amber-300 px-3 font-medium text-amber-200 hover:bg-amber-300/10 focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-amber-300 disabled:opacity-50"
            onClick={() => void installUpdate(state.update)}
          >
            更新して再起動
          </button>
        </>
      ) : (
        <button
          type="button"
          disabled={disabled || busy}
          className="min-h-10 rounded-lg border border-slate-700 px-3 text-slate-300 hover:border-slate-500 hover:bg-slate-800 focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-amber-300 disabled:opacity-50"
          onClick={() => void checkForUpdate()}
        >
          {state.kind === "checking" ? "確認中…" : "更新を確認"}
        </button>
      )}
      {state.kind === "latest" ? (
        <span role="status" className="text-slate-400">最新版です</span>
      ) : null}
      {state.kind === "installing" ? (
        <OperationProgress label={state.message} value={state.progress} className="w-full max-w-72" />
      ) : state.kind === "checking" ? (
        <OperationProgress label="更新を確認中…" />
      ) : null}
      {state.kind === "error" ? (
        <span role="alert" className="max-w-72 break-words text-rose-200">{state.message}</span>
      ) : null}
    </div>
  );
}
