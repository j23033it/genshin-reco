import { invoke } from "@tauri-apps/api/core";

export type TavilySettingsStatus = {
  configured: boolean;
};

export const readTavilySettingsStatus = (): Promise<TavilySettingsStatus> =>
  invoke<TavilySettingsStatus>("read_tavily_settings_status");

export const saveTavilyApiKey = (apiKey: string): Promise<TavilySettingsStatus> =>
  invoke<TavilySettingsStatus>("save_tavily_api_key", { apiKey });

export const testTavilyConnection = (): Promise<void> => invoke<void>("test_tavily_connection");

export const deleteTavilyApiKey = (): Promise<TavilySettingsStatus> =>
  invoke<TavilySettingsStatus>("delete_tavily_api_key");
