import { invoke } from "@tauri-apps/api/core";
import type { Gate0SmokeReport } from "./types";

export const runCodexGate0Smoke = (): Promise<Gate0SmokeReport> =>
  invoke<Gate0SmokeReport>("run_codex_gate0_smoke");
