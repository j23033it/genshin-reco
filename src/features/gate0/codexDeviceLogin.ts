import { invoke } from "@tauri-apps/api/core";
import type { CodexDeviceLoginChallenge, CodexLoginStatus } from "./types";

export const startCodexDeviceLogin = (): Promise<CodexDeviceLoginChallenge> =>
  invoke<CodexDeviceLoginChallenge>("start_codex_device_login");

export const readCodexLoginStatus = (): Promise<CodexLoginStatus> =>
  invoke<CodexLoginStatus>("read_codex_login_status");

export const cancelCodexDeviceLogin = (loginId: string): Promise<void> =>
  invoke<void>("cancel_codex_device_login", { loginId });
