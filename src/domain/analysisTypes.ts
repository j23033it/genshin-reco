export type BuildIntent =
  | "auto"
  | "on_field_damage"
  | "off_field_damage"
  | "reaction_trigger"
  | "support"
  | "sustain";

export type ReactionOwnership = "main" | "partial" | "none" | "unknown";
export type EnergyPriority = "damage_first" | "balanced" | "burst_stability";
export type SurvivabilityPriority = "normal" | "high";
export type TargetScope =
  | "character_sheet_unbuffed"
  | "character_sheet_with_static_team_effects"
  | "in_combat_conditional";
export type EvidenceGrade = "A" | "B" | "C";
export type EvidenceVerification = "host_exact_match" | "host_fuzzy_match" | "url_event_only" | "unverified";
export type ResolutionStatus = "resolved" | "needs_user_choice" | "unresolved";
export type AnalysisStatus =
  | "queued"
  | "starting_codex"
  | "researching"
  | "verifying_sources"
  | "reconciling"
  | "solving"
  | "persisting"
  | "succeeded"
  | "failed"
  | "cancelled"
  | "superseded"
  | "abandoned";
export type ResultValidity = "current" | "soft_stale" | "hard_stale" | "invalid";

export interface CharacterBuildIntent {
  role: BuildIntent;
  reactionOwnership: ReactionOwnership;
  energyPriority: EnergyPriority;
  survivabilityPriority: SurvivabilityPriority;
}

export interface PartyMemberInput {
  slotIndex: 0 | 1 | 2 | 3;
  characterId: string;
  weaponId: string;
  refinement: 1 | 2 | 3 | 4 | 5;
  constellation: 0 | 1 | 2 | 3 | 4 | 5 | 6;
  intent: CharacterBuildIntent;
}

export interface AnalysisVersions {
  catalogVersion: string;
  sourcePolicyVersion: string;
  promptVersion: string;
  schemaVersion: string;
  reconcilerVersion: string;
  solverVersion: string;
}

export interface FixedAssumptions {
  characterLevel: 90;
  weaponLevel: 90;
  artifactLevel: 20;
  artifactRarity: 5;
  sheetTiming: "pre_combat";
  finalAscension: true;
  allTalentsAvailable: true;
  witchTeachingWhenApplicable: true;
}

export interface AnalysisInput {
  partyId: string;
  partyName: string;
  gameVersion: string;
  members: [PartyMemberInput, PartyMemberInput, PartyMemberInput, PartyMemberInput];
  assumptions: FixedAssumptions;
  versions: AnalysisVersions;
}

export type ArtifactHalf =
  | { kind: "exact_set"; setId: string }
  | { kind: "effect_group"; effectGroupId: string };

export type ArtifactPlan =
  | { type: "four_piece"; setId: string }
  | { type: "two_plus_two"; first: ArtifactHalf; second: ArtifactHalf };

export interface BuildCondition {
  field: string;
  operator: "equals" | "not_equals" | "includes" | "gte" | "lte";
  value: string | number | boolean;
  description: string;
}

export interface StatPriority {
  stat: string;
  rank: number;
}

export interface TargetStatRange {
  stat: string;
  minimum: number | null;
  maximum: number | null;
  unit: "flat" | "percent";
  scope: TargetScope;
  note: string | null;
}

export interface MainStatPackage {
  id: string;
  sands: string;
  goblet: string;
  circlet: string;
  conditions: BuildCondition[];
  substatPriority: StatPriority[];
  targetStats: TargetStatRange[];
}

export interface SourceEvidence {
  sourcePageId: string;
  evidenceExcerpt: string | null;
  evidenceSummary: string;
  locator: {
    heading: string | null;
    section: string | null;
    textFragment: string | null;
  } | null;
  verification: EvidenceVerification;
  contentHash: string | null;
}

export type EvidenceClaimType =
  | "artifact_plan"
  | "main_stat_package"
  | "substat_priority"
  | "target_stat"
  | "role"
  | "team_interaction";

export type NormalizedClaimValue =
  | { kind: "artifact_plan"; value: ArtifactPlan }
  | { kind: "main_stat_package"; value: MainStatPackage }
  | { kind: "substat_priority"; value: StatPriority[] }
  | { kind: "target_stat"; value: TargetStatRange }
  | { kind: "role"; value: CharacterBuildIntent }
  | { kind: "team_interaction"; value: string };

export interface EvidenceClaim {
  claimType: EvidenceClaimType;
  normalizedValue: NormalizedClaimValue;
  conditions: BuildCondition[];
  evidence: SourceEvidence;
  evidenceGrade: EvidenceGrade;
}

export interface BuildVariant {
  id: string;
  characterId: string;
  artifactPlan: ArtifactPlan;
  mainStatPackage: MainStatPackage;
  conditions: BuildCondition[];
  teamBuffKeys: string[];
  evidenceClaims: EvidenceClaim[];
  sourceFamilyCount: number;
  conflictPenalty: number;
}

export interface CharacterBuildResolution {
  characterId: string;
  selectedVariantId: string | null;
  alternatives: BuildVariant[];
  reason: string;
}

export interface TeamBuildResolution {
  status: ResolutionStatus;
  members: CharacterBuildResolution[];
  warnings: string[];
}

export interface HostGeneratedIdentity {
  analysisRunId: string;
  partyCompositionHash: string;
  analysisInputHash: string;
  evidenceSnapshotHash: string;
  resultHash: string;
  checkedAt: string;
  createdAt: string;
}

export interface ResearchSourcePage {
  sourceUrl: string;
  title: string;
  publisher: string;
  gameVersion: string;
  updatedAt: string | null;
}

export interface ResearchLocator {
  heading: string | null;
  section: string | null;
  textFragment: string | null;
}

export interface ResearchEvidence {
  sourceUrl: string;
  evidenceExcerpt: string | null;
  evidenceSummary: string;
  locator: ResearchLocator | null;
}

export interface ResearchClaim {
  claimType: EvidenceClaimType;
  normalizedValue: NormalizedClaimValue;
  conditions: BuildCondition[];
  evidence: ResearchEvidence;
}

export interface ResearchBuildVariant {
  id: string;
  artifactPlan: ArtifactPlan;
  mainStatPackage: MainStatPackage;
  conditions: BuildCondition[];
  teamBuffKeys: string[];
  claims: ResearchClaim[];
}

/** Codex構造化出力には、Rust側で生成するID・hash・検証結果を含めない。 */
export interface CharacterResearchOutput {
  schemaVersion: "character-research-v1";
  characterId: string;
  sources: ResearchSourcePage[];
  variants: ResearchBuildVariant[];
  warnings: string[];
}
