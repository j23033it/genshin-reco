import { invoke } from "@tauri-apps/api/core";
import type { Catalog, StarRailCatalog } from "../../domain/catalogTypes";

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

export async function loadStarRailCatalog(): Promise<StarRailCatalog> {
  const catalog = window.__TAURI_INTERNALS__
    ? await invoke<StarRailCatalog>("load_star_rail_catalog")
    : await fetch("/data/star-rail/catalog.json").then(response => {
      if (!response.ok) throw new Error("スターレイルのカタログを読み込めません。");
      return response.json() as Promise<StarRailCatalog>;
    });
  if (catalog.game !== "star_rail" || catalog.schemaVersion !== "star-rail-catalog-v1") throw new Error("カタログのゲームが一致しません。");
  return catalog;
}
