import { DEMO_TEAM, DEMO_STAR_RAIL_TEAM } from "./demoData";
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
    async sendMessage(message, sessionId, game = "genshin") {
      const now = new Date().toISOString();
      const previous = sessionId ? sessions.get(sessionId) : undefined;
      const conversation: ResearchConversation = {
        game: previous?.game ?? game,
        sessionId: sessionId ?? crypto.randomUUID(),
        status: "ready",
        title: previous?.title ?? null,
        members: (game === "star_rail" ? DEMO_STAR_RAIL_TEAM : DEMO_TEAM).map((member, slotIndex) => ({
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
              : game === "star_rail" ? "これは固定の4人を使うデモです。星魂・光円錐・重畳と任意の遺物を選べます。指定なしでも進められます。" : "これは固定の4人を使うデモです。下の画面で凸と武器を選んでください。指定なしでも進められます。",
            createdAt: now,
          },
        ],
        createdAt: previous?.createdAt ?? now,
        updatedAt: now,
      };
      sessions.set(conversation.sessionId, conversation);
      return conversation;
    },
    async updateConditions(sessionId, members, title) {
      const previous = sessions.get(sessionId);
      if (!previous) throw new Error("デモの会話が見つかりません。");
      const now = new Date().toISOString();
      const conversation: ResearchConversation = {
        ...previous,
        status: "ready",
        members,
        title,
        updatedAt: now,
      };
      sessions.set(sessionId, conversation);
      return conversation;
    },
    async startResearch(sessionId) {
      if (!sessions.has(sessionId))
        throw new Error("デモの会話が見つかりません。");
      const conversation = sessions.get(sessionId)!;
      const now = new Date().toISOString();
      const demoMembers = conversation.game === "star_rail" ? DEMO_STAR_RAIL_TEAM.map(member => {
        const input = conversation.members.find(input => input.name === member.name)!;
        const base = member.starRail!;
        const tunnel = input.relics?.tunnel ?? base.tunnel;
        const names = tunnel.kind === "four_piece" ? [tunnel.set] : tunnel.sets;
        return { ...member, weapon: input.weapon ?? member.weapon, constellation: `${input.constellation ?? 0}凸`, starRail: {
          ...base, eidolon: input.constellation ?? 0, lightCone: input.weapon ?? member.weapon, superimposition: input.refinement ?? 1, tunnel,
          ornament: input.relics?.ornament ?? base.ornament,
          tunnelEvidence: names.map(set => ({ set, reason: "デモの表示例です。", conditions: "効果は本調査で確認します。", sourceUrls: [] })),
          ornamentEvidence: { ...base.ornamentEvidence, set: input.relics?.ornament ?? base.ornament },
        } };
      }) : DEMO_TEAM;
      const record: ResearchedTeamRecord = {
        game: conversation.game,
        inputMembers: structuredClone(conversation.members),
        teamReasoning: conversation.game === "star_rail" ? "表示確認用のデモです。実際の編成提案ではありません。" : null,
        teamId: conversation.teamId ?? crypto.randomUUID(),
        sessionId,
        title: sessions.get(sessionId)?.title || (conversation.game === "star_rail" ? "スターレイル編成（デモ）" : "アルレッキーノ蒸発編成（デモ）"),
        members: demoMembers,
        sources: [],
        warnings: ["表示確認用のサンプルです。実際の調査結果ではありません。"],
        createdAt: now,
        updatedAt: now,
      };
      records.set(record.teamId, record);
      sessions.set(sessionId, { ...conversation, title: record.title, teamId: record.teamId, status: "succeeded" });
      return record;
    },
    async cancelResearch() {},
    async listTeams(game = "genshin") {
      return [...records.values()].filter(record => (record.game ?? "genshin") === game).map((record) => ({
        game: record.game,
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
    async renameTeam(teamId, title) {
      const record = records.get(teamId);
      if (!record) throw new Error("この編成は見つかりません。");
      const updated = { ...record, title: title.trim(), updatedAt: new Date().toISOString() };
      records.set(teamId, updated);
      const conversation = sessions.get(record.sessionId);
      if (conversation) sessions.set(record.sessionId, { ...conversation, title: updated.title });
      return updated;
    },
    async subscribeProgress() {
      return () => {};
    },
  };
}
