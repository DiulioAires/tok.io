import { invoke } from "@tauri-apps/api/core";
import type { UsageDashboardData, SourceStatus, CodexQuota, ClaudeQuota, WidgetMode, WidgetPreferences, WidgetPosition } from "./types";

export const getUsageDashboard = () =>
  invoke<UsageDashboardData>("get_usage_dashboard");

export const getCodexQuota = () =>
  invoke<CodexQuota>("get_codex_quota");

export const getClaudeQuota = () =>
  invoke<ClaudeQuota>("get_claude_quota");

export const refreshUsageSources = () =>
  invoke<SourceStatus[]>("refresh_usage_sources");

export const clearUsageHistory = () =>
  invoke<void>("clear_usage_history");

export const getUsageSourceSettings = () =>
  invoke<Record<string, boolean>>("get_usage_source_settings");

export const setUsageSourceEnabled = (sourceId: string, enabled: boolean) =>
  invoke<void>("set_usage_source_enabled", { sourceId, enabled });

export const getProviderCredentialStatus = () =>
  invoke<Record<string, boolean>>("provider_credential_status");

export const saveProviderCredential = (provider: string, secret: string) =>
  invoke<void>("save_provider_credential", { provider, secret });

export const deleteProviderCredential = (provider: string) =>
  invoke<void>("delete_provider_credential", { provider });

export const getWidgetPreferences = () =>
  invoke<WidgetPreferences>("get_widget_preferences");

export const setWidgetMode = (mode: WidgetMode) =>
  invoke<WidgetPreferences>("set_widget_mode", { mode });

export const saveWidgetPosition = (windowLabel: "large" | "compact", position: WidgetPosition) =>
  invoke<WidgetPreferences>("save_widget_position", { windowLabel, position });
