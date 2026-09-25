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
  slotIndex: number;
  name: string;
  weapon?: string | null;
  constellation?: number | null;
  refinement?: number | null;
}

export interface ResearchConversation {
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
  teamId?: string | null;
  error?: string | null;
  createdAt: string;
  updatedAt: string;
}

export interface TeamMember extends ResearchMember {
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
    { label: string; value?: string | null; primary?: boolean }[] | null;
}

export interface ResearchedTeamSummary {
  teamId: string;
  title: string;
  updatedAt: string;
  memberImageUrls: (string | null)[];
  memberNames: string[];
}

export interface ResearchedTeamRecord {
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
  ): Promise<ResearchConversation>;
  updateConditions(
    sessionId: string,
    members: ResearchMemberInput[],
  ): Promise<ResearchConversation>;
  startResearch(sessionId: string): Promise<ResearchedTeamRecord>;
  cancelResearch(sessionId: string): Promise<void>;
  listTeams(): Promise<ResearchedTeamSummary[]>;
  loadConversation(sessionId: string): Promise<ResearchConversation | null>;
  loadTeam(teamId: string): Promise<ResearchedTeamRecord | null>;
  subscribeProgress(
    callback: (progress: ResearchProgress) => void,
  ): Promise<() => void>;
}
