import "@testing-library/jest-dom/vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import type {
  BuildVariant,
  CharacterBuildResolution,
  MainStatPackage,
  TeamBuildResolution,
} from "../../domain/analysisTypes";
import type { Catalog } from "../../domain/catalogTypes";
import type { PartyDraft } from "../party";
import { AnalysisProgressPanel } from "./AnalysisProgressPanel";
import { AnalysisNotesPanel, TeamResultPanel } from "./TeamResultPanel";

const catalog: Catalog = {
  schemaVersion: "catalog-v2",
  gameVersion: "5.8",
  catalogUpdatedAt: "2026-08-30",
  characters: [1, 2, 3, 4].map((number) => ({
    id: `character-${number}`,
    name: `キャラクター${number}`,
    element: "炎",
    weaponType: "片手剣",
    rarity: 5,
    imageUrl: `/character-${number}.png`,
  })),
  weapons: [1, 2, 3, 4].map((number) => ({
    id: `weapon-${number}`,
    name: `武器${number}`,
    weaponType: "片手剣",
    rarity: 5,
    imageUrl: `/weapon-${number}.png`,
  })),
  artifactSets: [
    {
      id: "set-crimson",
      name: "燃え盛る炎の魔女",
      teamBuffKey: null,
      twoPieceEffectGroupId: "pyro",
      twoPieceEffect: "炎元素ダメージ＋15％",
      fourPieceEffect: "過負荷などのダメージ強化",
      pieceImageUrls: { flower: "/crimson-flower.png", plume: "", sands: "", goblet: "", circlet: "" },
    },
  ],
};

const party: PartyDraft = {
  partyId: "party-1",
  name: "テスト編成",
  members: [0, 1, 2, 3].map((slotIndex) => ({
    slotIndex: slotIndex as 0 | 1 | 2 | 3,
    characterId: `character-${slotIndex + 1}`,
    weaponId: `weapon-${slotIndex + 1}`,
    constellation: slotIndex as 0 | 1 | 2 | 3 | 4 | 5 | 6,
    refinement: (slotIndex + 1) as 1 | 2 | 3 | 4 | 5,
  })) as PartyDraft["members"],
};

const mainStatPackage: MainStatPackage = {
  id: "package-1",
  sands: "元素チャージ効率",
  goblet: "炎元素ダメージ",
  circlet: "会心率",
  conditions: [],
  substatPriority: [
    { stat: "会心ダメージ", rank: 1 },
    { stat: "会心率", rank: 2 },
  ],
  targetStats: [
    {
      stat: "元素チャージ効率",
      minimum: 180,
      maximum: null,
      unit: "percent",
      scope: "in_combat_conditional",
      note: "爆発を毎回使う条件",
    },
  ],
};

function createVariant(id: string): BuildVariant {
  return {
    id,
    characterId: "character-1",
    artifactPlan: { type: "four_piece", setId: "set-crimson" },
    mainStatPackage,
    conditions: [],
    teamBuffKeys: [],
    evidenceClaims: [],
    sourceFamilyCount: 1,
    conflictPenalty: 0,
  };
}

function createMember(characterId: string, selectedVariantId: string | null): CharacterBuildResolution {
  return {
    characterId,
    selectedVariantId,
    alternatives: [createVariant(`${characterId}-variant-a`), createVariant(`${characterId}-variant-b`)],
    reason: "チーム内の役割と根拠の整合性が高いためです。",
  };
}

function createResolution(status: TeamBuildResolution["status"], selectedVariantId: string | null): TeamBuildResolution {
  return {
    status,
    members: [
      createMember("character-1", selectedVariantId),
      createMember("character-2", selectedVariantId),
      createMember("character-3", selectedVariantId),
      createMember("character-4", selectedVariantId),
    ],
    warnings: ["条件付きの目標値を含みます。"],
  };
}

describe("AnalysisProgressPanel", () => {
  it("ライブリージョン、工程別状態、キャンセル、以前の結果を表示する", async () => {
    const user = userEvent.setup();
    const onCancel = vi.fn();

    render(
      <AnalysisProgressPanel
        status="researching"
        characterSteps={[
          { characterId: "character-1", characterName: "キャラクター1", status: "researching" },
          { characterId: "character-2", characterName: "キャラクター2", status: "queued" },
        ]}
        lastResultValidity="soft_stale"
        onCancel={onCancel}
      />,
    );

    expect(screen.getByRole("status")).toHaveTextContent("根拠を調査しています");
    expect(screen.getByTestId("analysis-stale-banner")).toHaveTextContent("以前の分析結果を表示中");
    expect(screen.getByText("調査中")).toBeInTheDocument();
    expect(screen.getByText("待機中")).toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: "分析をキャンセル" }));
    expect(onCancel).toHaveBeenCalledOnce();
  });

  it("失敗時はエラーの近くに再実行の説明を表示する", () => {
    render(
      <AnalysisProgressPanel
        status="failed"
        characterSteps={[{ characterId: "character-1", status: "failed", error: "根拠を取得できませんでした。" }]}
        lastResultValidity={null}
        error="分析サーバーに接続できませんでした。"
      />,
    );

    expect(screen.getByRole("alert")).toHaveTextContent("分析サーバーに接続できませんでした");
    expect(screen.getByRole("alert")).toHaveTextContent("もう一度分析を実行してください");
  });
});

describe("TeamResultPanel", () => {
  it("空状態では次の操作を1つだけ案内する", () => {
    render(<TeamResultPanel catalog={catalog} party={null} resolution={null} validity="current" />);

    expect(screen.getByTestId("team-result-empty")).toHaveTextContent("保存済み編成を選択するか");
    expect(screen.queryByRole("button")).not.toBeInTheDocument();
  });

  it("確定結果で4人分のカード、選択候補、警告、根拠と目標値を表示する", () => {
    render(<TeamResultPanel catalog={catalog} party={party} resolution={createResolution("resolved", "character-1-variant-a")} validity="current" />);

    expect(screen.getByText("分析結果：確定")).toBeInTheDocument();
    expect(screen.getAllByTestId("character-result-card")).toHaveLength(4);
    expect(screen.getAllByText("キャラクター1").length).toBeGreaterThan(0);
    expect(screen.getAllByText("武器1").length).toBeGreaterThan(0);
    expect(screen.getAllByText("燃え盛る炎の魔女").length).toBeGreaterThan(0);
    expect(screen.getAllByText(/砂：元素チャージ効率/).length).toBeGreaterThan(0);
    expect(screen.getAllByText(/優先サブ：会心ダメージ > 会心率/).length).toBeGreaterThan(0);
    expect(screen.queryByText(/set-crimson/)).not.toBeInTheDocument();
    expect(screen.queryByText(/4セット/)).not.toBeInTheDocument();
    expect(screen.queryByText(/EvidenceGrade|verification|根拠|推薦理由/)).not.toBeInTheDocument();
    expect(screen.getAllByText("C0").length).toBeGreaterThan(0);
    expect(screen.getAllByText("R1").length).toBeGreaterThan(0);
  });

  it("候補未選択時は候補ボタンをキーボード操作し、選択を通知する", async () => {
    const user = userEvent.setup();
    const onChooseVariant = vi.fn();
    render(
        <TeamResultPanel
          catalog={catalog}
          party={party}
        resolution={createResolution("needs_user_choice", null)}
        validity="current"
        onChooseVariant={onChooseVariant}
      />,
    );

    expect(screen.getByText("分析結果：候補を選択してください")).toBeInTheDocument();
    const candidateButton = screen.getAllByRole("button", { name: "候補1を選択" })[0];
    candidateButton.focus();
    await user.keyboard("{Enter}");

    expect(onChooseVariant).toHaveBeenCalledWith("character-1", "character-1-variant-a");
  });

  it("未解決状態を日本語で表示し警告を残す", () => {
    render(<TeamResultPanel catalog={catalog} party={party} resolution={createResolution("unresolved", null)} validity="hard_stale" />);

    expect(screen.getByText("分析結果：解決できませんでした")).toBeInTheDocument();
    expect(screen.getByRole("status")).toHaveTextContent("補足事項を確認してください。");
  });
});

describe("AnalysisNotesPanel", () => {
  it("警告・候補の条件・目標注記・古い結果を重複なく表示する", () => {
    const resolution = createResolution("resolved", "character-1-variant-a");
    resolution.warnings = ["条件を確認してください。", "条件を確認してください。"];
    resolution.members[0].alternatives[0].conditions = [
      { field: "energy", operator: "gte", value: 180, description: "元素チャージ効率を満たす" },
    ];
    resolution.members[0].alternatives[0].mainStatPackage.targetStats[0].note = "爆発を毎回使う条件";

    render(<AnalysisNotesPanel resolution={resolution} validity="hard_stale" />);

    expect(screen.getByTestId("analysis-notes-panel")).toHaveTextContent("条件を確認してください。");
    expect(screen.getByTestId("analysis-notes-panel")).toHaveTextContent("適用条件：元素チャージ効率を満たす");
    expect(screen.getByTestId("analysis-notes-panel")).toHaveTextContent("目標値の注記（元素チャージ効率）：爆発を毎回使う条件");
    expect(screen.getByTestId("analysis-notes-panel")).toHaveTextContent("古い分析結果です");
    expect(screen.getAllByText("条件を確認してください。")).toHaveLength(1);
  });

  it("補足がない場合は既定文言を表示する", () => {
    render(<AnalysisNotesPanel resolution={null} validity="current" />);
    expect(screen.getByText("補足事項はありません")).toBeInTheDocument();
  });
});
