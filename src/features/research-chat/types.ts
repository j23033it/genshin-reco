export type GameId = "genshin" | "star_rail";
export type TunnelSelection = { kind: "four_piece"; set: string } | { kind: "two_plus_two"; sets: [string, string] };
export interface RelicInput { tunnel: TunnelSelection | null; ornament: string | null }
export interface SetEvidence { set: string; reason: string; conditions: string; sourceUrls: string[]; imageUrl?: string | null }
export interface StarRailBuild {
  eidolon: number; lightCone: string; superimposition: number;
  tunnel: TunnelSelection; ornament: string;
  tunnelEvidence: SetEvidence[]; ornamentEvidence: SetEvidence;
}

export type ResearchStage =
  | "empty"
  | "collecting"
  | "ready"
  | "researching"
  | "result"
  | "error"
  | "cancelled";

export interface ResearchMember {
  id: string;
  name: string;
  imageUrl?: string | null;
}

export interface ResearchMemberInput {
  relics?: RelicInput | null;
  // In Star Rail: weapon=light cone, constellation=eidolon, refinement=superimposition.
  slotIndex: number;
  name: string;
  weapon?: string | null;
  constellation?: number | null;
  refinement?: number | null;
}

export interface ResearchConversation {
  game?: GameId; // Missing discriminator belongs to legacy Genshin data.
  sessionId: string;
  status:
    | "collecting"
    | "ready"
    | "researching"
    | "succeeded"
    | "failed"
    | "cancelled";
  messages: {
    role: "user" | "assistant";
    content: string;
    createdAt: string;
  }[];
  members: ResearchMemberInput[];
  title?: string | null;
  teamId?: string | null;
  error?: string | null;
  createdAt: string;
  updatedAt: string;
}

export interface TeamMember extends ResearchMember {
  refinement?: number | null;
  starRail?: StarRailBuild | null;
  element: string;
  role: string;
  constellation: string;
  weapon: string;
  weaponImageUrl?: string | null;
  artifact: string;
  artifactImageUrl?: string | null;
  mainStats: string;
  subStats: string;
  targetStats?:
    { label: string; value?: string | null; primary?: boolean; note?: string | null }[] | null;
}

export interface ResearchedTeamSummary {
  game?: GameId; // Missing discriminator belongs to legacy Genshin data.
  teamId: string;
  title: string;
  updatedAt: string;
  memberImageUrls: (string | null)[];
  memberNames: string[];
}

export interface ResearchedTeamRecord {
  inputMembers?: ResearchMemberInput[] | null;
  teamReasoning?: string | null;
  game?: GameId; // Missing discriminator belongs to legacy Genshin data.
  teamId: string;
  sessionId: string;
  title: string;
  gameVersion?: string | null;
  members: TeamMember[];
  sources: { title: string; url: string }[];
  warnings: string[];
  createdAt: string;
  updatedAt: string;
}

export interface ResearchProgress {
  game?: GameId; // Missing discriminator belongs to legacy Genshin data.
  sessionId: string;
  stage: string;
  detail: string;
  memberName?: string | null;
}

export interface ResearchRepository {
  mode: "tauri" | "demo";
  sendMessage(
    message: string,
    sessionId?: string,
    game?: GameId,
  ): Promise<ResearchConversation>;
  updateConditions(
    sessionId: string,
    members: ResearchMemberInput[],
    title: string | null,
  ): Promise<ResearchConversation>;
  startResearch(sessionId: string): Promise<ResearchedTeamRecord>;
  cancelResearch(sessionId: string): Promise<void>;
  listTeams(game?: GameId): Promise<ResearchedTeamSummary[]>;
  loadConversation(sessionId: string): Promise<ResearchConversation | null>;
  loadTeam(teamId: string): Promise<ResearchedTeamRecord | null>;
  renameTeam(teamId: string, title: string): Promise<ResearchedTeamRecord>;
  subscribeProgress(
    callback: (progress: ResearchProgress) => void,
  ): Promise<() => void>;
}
