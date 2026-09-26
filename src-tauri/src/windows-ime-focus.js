/* global window, document, HTMLTextAreaElement */
(() => {
  if (window.__genshinImeFocusInstalled) return;
  const invoke = window.__TAURI_INTERNALS__?.invoke;
  if (!invoke) return;
  window.__genshinImeFocusInstalled = true;

  let composing = false;
  let pending = false;
  document.addEventListener("compositionstart", () => { composing = true; }, true);
  document.addEventListener("compositionend", () => { composing = false; }, true);
  document.addEventListener("pointerup", (event) => {
    const target = event.target;
    // マウスで編成欄を選んだ時だけ再接続する。変換中や別の入力先には触れない。
    if (
      event.button !== 0 ||
      !(target instanceof HTMLTextAreaElement) ||
      target.id !== "team-request" ||
      target.disabled ||
      document.activeElement !== target ||
      composing ||
      pending
    ) return;
    pending = true;
    Promise.resolve(invoke("repair_windows_ime_focus", { requestedAt: Date.now() }))
      .catch(() => {})
      .finally(() => { pending = false; });
  }, true);
})();
