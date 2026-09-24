export type UsageBucket = {
  id: string;
  sourceId: string;
  usageKind: string;
  periodStart: number;
  periodEnd: number;
  model: string | null;
  sessionKey: string | null;
  inputTokens: number | null;
  outputTokens: number | null;
  cachedInputTokens: number | null;
  totalTokens: number | null;
  amount: number | null;
  currency: string | null;
  confidence: string;
  observedAt: number;
};

export type SourceStatus = {
  sourceId: string;
  state: "connected" | "partial" | "stale" | "unavailable";
  lastSuccessfulRefresh: number | null;
  message: string | null;
};

export type UsageDashboardData = {
  generatedAt: number;
  buckets: UsageBucket[];
  sources: SourceStatus[];
};

export type CodexQuotaWindow = {
  usedPercent: number;
  resetsAt: number;
};

export type CodexQuota = {
  fiveHour: CodexQuotaWindow | null;
  weekly: CodexQuotaWindow | null;
  planType: string | null;
  refreshedAt: number;
};

export type ClaudeQuota = {
  fiveHour: CodexQuotaWindow | null;
  weekly: CodexQuotaWindow | null;
  refreshedAt: number;
};

export type WidgetMode = "large" | "compact";

export type WidgetPosition = {
  x: number;
  y: number;
};

export type WidgetPreferences = {
  mode: WidgetMode;
  large_position: WidgetPosition | null;
  compact_position: WidgetPosition | null;
};
