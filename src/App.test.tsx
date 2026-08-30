import "@testing-library/jest-dom/vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import App from "./App";
import type { Catalog } from "./domain/catalogTypes";
import { loadCatalog } from "./features/catalog";

vi.mock("./features/catalog", () => ({
  loadCatalog: vi.fn(),
}));

const loadCatalogMock = vi.mocked(loadCatalog);
const catalog: Catalog = {
  schemaVersion: "catalog-v2",
  gameVersion: "7.0",
  catalogUpdatedAt: "2026-08-30",
  characters: [
    {
      id: "char-a",
      name: "キャラA",
      element: "炎",
      weaponType: "片手剣",
      rarity: 5,
      imageUrl: "https://example.com/a.png",
    },
  ],
  weapons: [
    {
      id: "weapon-a",
      name: "武器A",
      weaponType: "片手剣",
      rarity: 5,
      imageUrl: "https://example.com/w.png",
    },
  ],
  artifactSets: [],
};

describe("アプリワークスペース", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    loadCatalogMock.mockResolvedValue(catalog);
  });

  it("凍結カタログ読み込み後に編成ビルダーを表示する", async () => {
    render(<App />);

    expect(screen.getByRole("status")).toHaveTextContent("凍結カタログ");
    expect(await screen.findByRole("heading", { name: "4人編成を作成" })).toBeInTheDocument();
    expect(screen.getByText("Ver.7.0 / 1キャラ")).toBeInTheDocument();
    expect(screen.getByRole("navigation", { name: "主要画面" })).toBeInTheDocument();
  });

  it("途中の編成を保存してサイドバーから開ける", async () => {
    const user = userEvent.setup();
    render(<App />);
    const name = await screen.findByRole("textbox", { name: "編成名" });

    await user.type(name, "蒸発チーム");
    await user.click(screen.getByRole("button", { name: "保存" }));

    expect(screen.getByText("「蒸発チーム」を保存しました。")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "蒸発チーム" })).toBeInTheDocument();
    expect(screen.getByText("1件")).toBeInTheDocument();
  });

  it("Codex設定からGate0を開ける", async () => {
    const user = userEvent.setup();
    render(<App />);

    await user.click(await screen.findByRole("button", { name: "Codex設定" }));

    expect(screen.getByRole("heading", { name: "Codexの環境を確認" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "環境を確認" })).toBeEnabled();
  });

  it("カタログ読み込み失敗を表示して再実行できる", async () => {
    loadCatalogMock.mockRejectedValueOnce(new Error("カタログが壊れています"));
    const user = userEvent.setup();
    render(<App />);

    expect(await screen.findByRole("alert")).toHaveTextContent("カタログが壊れています");
    await user.click(screen.getByRole("button", { name: "再読み込み" }));

    expect(await screen.findByRole("heading", { name: "4人編成を作成" })).toBeInTheDocument();
    expect(loadCatalogMock).toHaveBeenCalledTimes(2);
  });
});
