import { useEffect, useState, type FormEvent } from "react";
import { CheckCircle2, KeyRound, LoaderCircle, Trash2 } from "lucide-react";
import { cn } from "../../lib/cn";
import {
  deleteTavilyApiKey,
  readTavilySettingsStatus,
  saveTavilyApiKey,
  testTavilyConnection,
  type TavilySettingsStatus,
} from "./tavilySettings";

type PendingAction = "load" | "save" | "test" | "delete" | null;

const errorMessage = (error: unknown) =>
  error instanceof Error ? error.message : typeof error === "string" ? error : "Tavily設定の操作に失敗しました。";

export function TavilySettingsPanel() {
  const desktopRuntime = Boolean(window.__TAURI_INTERNALS__);
  const [status, setStatus] = useState<TavilySettingsStatus | null>(
    desktopRuntime ? null : { configured: false },
  );
  const [apiKey, setApiKey] = useState("");
  const [pending, setPending] = useState<PendingAction>(desktopRuntime ? "load" : null);
  const [notice, setNotice] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (!desktopRuntime) {
      return;
    }
    let active = true;
    readTavilySettingsStatus()
      .then((next) => {
        if (active) setStatus(next);
      })
      .catch((cause: unknown) => {
        if (active) setError(errorMessage(cause));
      })
      .finally(() => {
        if (active) setPending(null);
      });
    return () => {
      active = false;
    };
  }, [desktopRuntime]);

  const handleSave = async (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault();
    setError(null);
    setNotice(null);
    setPending("save");
    try {
      const next = await saveTavilyApiKey(apiKey);
      setStatus(next);
      setApiKey("");
      setNotice("APIキーをOSの資格情報ストアへ保存しました。");
    } catch (cause) {
      setError(errorMessage(cause));
    } finally {
      setPending(null);
    }
  };

  const handleTest = async () => {
    setError(null);
    setNotice(null);
    setPending("test");
    try {
      await testTavilyConnection();
      setNotice("Tavily APIへの接続を確認できました。");
    } catch (cause) {
      setError(errorMessage(cause));
    } finally {
      setPending(null);
    }
  };

  const handleDelete = async () => {
    setError(null);
    setNotice(null);
    setPending("delete");
    try {
      const next = await deleteTavilyApiKey();
      setStatus(next);
      setApiKey("");
      setNotice("保存済みのAPIキーを削除しました。");
    } catch (cause) {
      setError(errorMessage(cause));
    } finally {
      setPending(null);
    }
  };

  const busy = pending !== null;
  const configured = status?.configured === true;

  return (
    <section
      className="mx-auto max-w-4xl rounded-2xl border border-slate-800 bg-slate-900/40 p-6 shadow-sm sm:p-8"
      aria-labelledby="tavily-settings-heading"
    >
      <div className="flex flex-wrap items-start justify-between gap-4">
        <div className="max-w-2xl">
          <p className="text-sm font-semibold text-amber-400">検索の高速化</p>
          <h2 id="tavily-settings-heading" className="mt-2 text-balance text-2xl font-semibold sm:text-3xl">
            Tavily API
          </h2>
          <p className="mt-3 text-pretty leading-7 text-slate-300">
            調査前に関連ページを検索・抽出し、Codexへ必要な本文だけ渡します。不足した情報は従来のWeb調査で補います。
          </p>
        </div>
        <span
          className={cn(
            "rounded-full border px-3 py-1 text-sm font-semibold",
            pending === "load"
              ? "border-slate-700 bg-slate-950 text-slate-300"
              : configured
                ? "border-emerald-400/50 bg-emerald-400/10 text-emerald-200"
                : "border-amber-400/50 bg-amber-400/10 text-amber-200",
          )}
          role="status"
          aria-live="polite"
        >
          {pending === "load" ? "確認中" : configured ? "設定済み" : "未設定"}
        </span>
      </div>

      <form className="mt-7 max-w-2xl" onSubmit={handleSave}>
        <label htmlFor="tavily-api-key" className="font-semibold text-slate-100">
          Tavily APIキー
        </label>
        <p id="tavily-api-key-help" className="mt-1 text-pretty text-sm leading-6 text-slate-400">
          キーはOSの資格情報ストアへ保存し、この画面には再表示しません。
        </p>
        <div className="relative mt-3">
          <KeyRound aria-hidden="true" className="pointer-events-none absolute left-3 top-3 text-slate-500" size={19} />
          <input
            id="tavily-api-key"
            type="password"
            value={apiKey}
            onChange={(event) => setApiKey(event.target.value)}
            className="min-h-11 w-full rounded-lg border border-slate-700 bg-slate-950 py-2 pl-10 pr-3 text-slate-100 placeholder:text-slate-600 focus:border-amber-400 focus:outline-none"
            placeholder={configured ? "新しいキーへ変更する場合のみ入力" : "tvly-…"}
            aria-describedby="tavily-api-key-help"
            autoComplete="off"
            disabled={busy}
          />
        </div>

        <div className="mt-4 flex flex-wrap gap-3">
          <button
            type="submit"
            className="inline-flex min-h-11 items-center justify-center gap-2 rounded-lg bg-amber-400 px-5 py-2 font-semibold text-slate-950 hover:bg-amber-300 disabled:cursor-not-allowed disabled:opacity-50"
            disabled={busy || apiKey.trim().length === 0}
          >
            {pending === "save" ? <LoaderCircle aria-hidden="true" size={18} /> : null}
            {configured ? "APIキーを更新" : "APIキーを保存"}
          </button>
          <button
            type="button"
            className="inline-flex min-h-11 items-center justify-center gap-2 rounded-lg border border-slate-700 px-5 py-2 font-semibold text-slate-200 hover:bg-slate-800 disabled:cursor-not-allowed disabled:opacity-50"
            onClick={() => void handleTest()}
            disabled={busy || !configured}
          >
            {pending === "test" ? <LoaderCircle aria-hidden="true" size={18} /> : null}
            接続を確認
          </button>
          {configured ? (
            <button
              type="button"
              className="inline-flex min-h-11 items-center justify-center gap-2 rounded-lg px-4 py-2 font-semibold text-rose-200 hover:bg-rose-400/10 disabled:cursor-not-allowed disabled:opacity-50"
              onClick={() => void handleDelete()}
              disabled={busy}
            >
              <Trash2 aria-hidden="true" size={18} />
              削除
            </button>
          ) : null}
        </div>
      </form>

      {notice ? (
        <p className="mt-4 flex items-start gap-2 text-pretty text-sm leading-6 text-emerald-200" role="status">
          <CheckCircle2 aria-hidden="true" className="mt-0.5 shrink-0" size={18} />
          {notice}
        </p>
      ) : null}
      {error ? (
        <p className="mt-4 border-l-2 border-rose-400 pl-3 text-pretty text-sm leading-6 text-rose-200" role="alert">
          {error}
        </p>
      ) : null}
      {!desktopRuntime ? (
        <p className="mt-4 border-l-2 border-slate-600 pl-3 text-pretty text-sm leading-6 text-slate-400">
          APIキーの保存と接続確認はデスクトップアプリで利用できます。
        </p>
      ) : null}
    </section>
  );
}
