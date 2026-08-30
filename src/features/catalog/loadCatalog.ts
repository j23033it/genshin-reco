import { invoke } from "@tauri-apps/api/core";
import type { Catalog } from "../../domain/catalogTypes";

declare global {
  interface Window {
    __TAURI_INTERNALS__?: unknown;
  }
}

/**
 * 製品ではTrusted Coreが検証した埋込カタログを読み込む。
 * Vite単体の開発・テスト時だけ同じ凍結JSONへフォールバックする。
 */
export async function loadCatalog(): Promise<Catalog> {
  if (window.__TAURI_INTERNALS__) {
    return invoke<Catalog>("load_catalog");
  }

  const response = await fetch("/data/catalog.json");
  if (!response.ok) {
    throw new Error(`カタログを読み込めませんでした（${response.status}）`);
  }
  return response.json() as Promise<Catalog>;
}
