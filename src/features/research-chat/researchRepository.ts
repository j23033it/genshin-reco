import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type { ResearchProgress, ResearchRepository } from "./types";

export const researchRepository: ResearchRepository = {
  mode: "tauri",
  sendMessage: (message, sessionId) =>
    invoke("send_on_demand_message", { sessionId, message }),
  updateConditions: (sessionId, members) =>
    invoke("update_on_demand_conditions", { sessionId, members }),
  startResearch: (sessionId) =>
    invoke("start_on_demand_research", { sessionId }),
  cancelResearch: (sessionId) =>
    invoke("cancel_on_demand_research", { sessionId }),
  listTeams: () => invoke("list_researched_teams"),
  loadConversation: (sessionId) =>
    invoke("load_on_demand_conversation", { sessionId }),
  loadTeam: (teamId) => invoke("load_researched_team", { teamId }),
  subscribeProgress: (callback) =>
    listen<ResearchProgress>("on-demand-research-progress", ({ payload }) =>
      callback(payload),
    ),
};
