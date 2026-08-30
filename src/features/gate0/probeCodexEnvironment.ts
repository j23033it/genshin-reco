import { invoke } from "@tauri-apps/api/core";
import type { Gate0ProbeReport } from "./types";

export const probeCodexEnvironment = (): Promise<Gate0ProbeReport> =>
  invoke<Gate0ProbeReport>("probe_codex_environment");
