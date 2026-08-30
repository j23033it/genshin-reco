import type { AnalysisStatus, ResultValidity } from "../../domain/analysisTypes";

/** キャラクターごとの分析工程。割合ではなく、現在の工程を表す。 */
export type AnalysisCharacterStepStatus =
  | "queued"
  | "researching"
  | "verifying"
  | "reconciling"
  | "solving"
  | "completed"
  | "failed"
  | "cancelled";

/**
 * 分析中のキャラクター1人分の状態。
 * `stage` は外部イベント名との境界で受け付け、表示上は `status` と同じ工程として扱う。
 */
export type CharacterAnalysisStep =
  | {
      characterId: string;
      characterName?: string;
      name?: string;
      status: AnalysisCharacterStepStatus;
      detail?: string;
      error?: string;
    }
  | {
      characterId: string;
      characterName?: string;
      name?: string;
      stage: AnalysisCharacterStepStatus;
      detail?: string;
      error?: string;
    };

/** パネルが受け取る、キャラクター別の進捗モデル。 */
export type AnalysisProgress = readonly CharacterAnalysisStep[];

export interface AnalysisProgressPanelProps {
  status: AnalysisStatus;
  characterSteps: AnalysisProgress;
  lastResultValidity: ResultValidity | null;
  onCancel?: () => void;
  error?: string | null;
}
