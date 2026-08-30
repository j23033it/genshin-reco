import "@testing-library/jest-dom/vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import type {
  BuildVariant,
  CharacterBuildResolution,
  EvidenceClaim,
  MainStatPackage,
  TeamBuildResolution,
} from "../../domain/analysisTypes";
import { AnalysisProgressPanel } from "./AnalysisProgressPanel";
import { TeamResultPanel } from "./TeamResultPanel";

const evidenceClaim: EvidenceClaim = {
  claimType: "target_stat",
  normalizedValue: {
    kind: "target_stat",
    value: {
      stat: "元素チャージ効率",
      minimum: 180,
      maximum: null,
      unit: "percent",
      scope: "in_combat_conditional",
      note: "爆発を毎回使う条件",
    },
  },
  conditions: [],
  evidence: {
    sourcePageId: "source-1",
    evidenceExcerpt: "爆発を毎回使えるように調整する。",
    evidenceSummary: "爆発の回転を優先する根拠です。",
    locator: null,
    verification: "host_exact_match",
    contentHash: "hash-1",
  },
  evidenceGrade: "A",
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
    evidenceClaims: [evidenceClaim],
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
    render(<TeamResultPanel resolution={null} validity="current" />);

    expect(screen.getByTestId("team-result-empty")).toHaveTextContent("編成を分析して");
    expect(screen.queryByRole("button")).not.toBeInTheDocument();
  });

  it("確定結果で4人分のカード、選択候補、警告、根拠と目標値を表示する", () => {
    render(<TeamResultPanel resolution={createResolution("resolved", "character-1-variant-a")} validity="current" />);

    expect(screen.getByText("分析結果：確定")).toBeInTheDocument();
    expect(screen.getAllByTestId("character-result-card")).toHaveLength(4);
    expect(screen.getByText("選択中の候補：character-1-variant-a")).toBeInTheDocument();
    expect(screen.getAllByText("代替候補")).toHaveLength(4);
    expect(screen.getAllByText("4セット：set-crimson").length).toBeGreaterThan(0);
    expect(screen.getAllByText(/砂・杯・冠/).length).toBeGreaterThan(0);
    expect(screen.getAllByText("元素チャージ効率", { selector: "th" }).length).toBeGreaterThan(0);
    expect(screen.getAllByText(/in_combat_conditional/).length).toBeGreaterThan(0);
    expect(screen.getAllByText("EvidenceGrade: A").length).toBeGreaterThan(0);
    expect(screen.getAllByText(/verification: host_exact_match/).length).toBeGreaterThan(0);
    expect(screen.getByText("条件付きの目標値を含みます。")).toBeInTheDocument();
  });

  it("候補未選択時は候補ボタンをキーボード操作し、選択を通知する", async () => {
    const user = userEvent.setup();
    const onChooseVariant = vi.fn();
    render(
      <TeamResultPanel
        resolution={createResolution("needs_user_choice", null)}
        validity="current"
        onChooseVariant={onChooseVariant}
      />,
    );

    expect(screen.getByText("分析結果：候補を選択してください")).toBeInTheDocument();
    const candidateButton = screen.getByRole("button", { name: "候補 character-1-variant-a を選択" });
    candidateButton.focus();
    await user.keyboard("{Enter}");

    expect(onChooseVariant).toHaveBeenCalledWith("character-1", "character-1-variant-a");
  });

  it("未解決状態を日本語で表示し警告を残す", () => {
    render(<TeamResultPanel resolution={createResolution("unresolved", null)} validity="hard_stale" />);

    expect(screen.getByText("分析結果：解決できませんでした")).toBeInTheDocument();
    expect(screen.getByTestId("team-result-stale-banner")).toHaveTextContent("再分析が必要");
    expect(screen.getByText("条件付きの目標値を含みます。")).toBeInTheDocument();
  });
});
