import { DEMO_TEAM } from "./demoData";
import type {
  ResearchConversation,
  ResearchRepository,
  ResearchedTeamRecord,
} from "./types";

// ブラウザで明示的に選択した場合だけ使う。実際の調査・永続保存は行わない。
export function createDemoResearchRepository(): ResearchRepository {
  const sessions = new Map<string, ResearchConversation>();
  const records = new Map<string, ResearchedTeamRecord>();
  return {
    mode: "demo",
    async sendMessage(message, sessionId) {
      const now = new Date().toISOString();
      const previous = sessionId ? sessions.get(sessionId) : undefined;
      const conversation: ResearchConversation = {
        sessionId: sessionId ?? crypto.randomUUID(),
        status: "ready",
        members: DEMO_TEAM.map((member, slotIndex) => ({
          slotIndex,
          name: member.name,
        })),
        messages: [
          ...(previous?.messages ?? []),
          { role: "user", content: message, createdAt: now },
          {
            role: "assistant",
            content: previous
              ? "デモの条件を確認しました。調査を開始できます。"
              : "これは固定の4人を使うデモです。下の画面で凸と武器を選んでください。指定なしでも進められます。",
            createdAt: now,
          },
        ],
        createdAt: previous?.createdAt ?? now,
        updatedAt: now,
      };
      sessions.set(conversation.sessionId, conversation);
      return conversation;
    },
    async updateConditions(sessionId, members) {
      const previous = sessions.get(sessionId);
      if (!previous) throw new Error("デモの会話が見つかりません。");
      const now = new Date().toISOString();
      const conversation: ResearchConversation = {
        ...previous,
        status: "ready",
        members,
        updatedAt: now,
      };
      sessions.set(sessionId, conversation);
      return conversation;
    },
    async startResearch(sessionId) {
      if (!sessions.has(sessionId))
        throw new Error("デモの会話が見つかりません。");
      const now = new Date().toISOString();
      const record: ResearchedTeamRecord = {
        teamId: crypto.randomUUID(),
        sessionId,
        title: "アルレッキーノ蒸発編成（デモ）",
        members: DEMO_TEAM,
        sources: [],
        warnings: ["表示確認用のサンプルです。実際の調査結果ではありません。"],
        createdAt: now,
        updatedAt: now,
      };
      records.set(record.teamId, record);
      return record;
    },
    async cancelResearch() {},
    async listTeams() {
      return [...records.values()].map((record) => ({
        teamId: record.teamId,
        title: record.title,
        updatedAt: record.updatedAt,
        memberNames: record.members.map((member) => member.name),
        memberImageUrls: record.members.map(
          (member) => member.imageUrl ?? null,
        ),
      }));
    },
    async loadConversation(sessionId) {
      return sessions.get(sessionId) ?? null;
    },
    async loadTeam(teamId) {
      return records.get(teamId) ?? null;
    },
    async subscribeProgress() {
      return () => {};
    },
  };
}
