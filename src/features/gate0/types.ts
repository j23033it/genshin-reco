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

export interface CodexDeviceLoginChallenge {
  loginId: string;
  verificationUrl: string;
  userCode: string;
}

export interface CodexLoginStatus {
  authenticated: boolean;
  account: Gate0Account;
  loginCompleted: boolean | null;
  loginError: string | null;
}

export interface Gate0SmokeReport {
  structuredOutputValid: boolean;
  webSearchObserved: boolean;
  cancellationObserved: boolean;
  instructionSourcesSupported: boolean;
  modelRerouted: boolean;
  reroutedFrom: string | null;
  reroutedTo: string | null;
}
