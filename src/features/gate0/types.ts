export interface Gate0Account {
  authMode: string | null;
  planType: string | null;
  requiresOpenaiAuth: boolean;
}

export interface Gate0ProbeReport {
  codexPath: string;
  codexVersion: string;
  versionSupported: boolean;
  appServerInitialized: boolean;
  isolatedHome: string;
  platformFamily: string | null;
  platformOs: string | null;
  account: Gate0Account | null;
  rateLimitsAvailable: boolean;
  diagnostics: string[];
}
