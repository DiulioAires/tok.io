import { useCallback, useEffect, useState } from "react";
import { emitTo, listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import {
  clearUsageHistory,
  deleteProviderCredential,
  getClaudeQuota,
  getCodexQuota,
  getProviderCredentialStatus,
  getUsageDashboard,
  getUsageSourceSettings,
  refreshUsageSources,
  saveProviderCredential,
  setUsageSourceEnabled,
} from "./usageApi";
import type { ClaudeQuota, CodexQuota, UsageDashboardData } from "./types";

export type UsageSnapshot = {
  data: UsageDashboardData | null;
  sourceSettings: Record<string, boolean>;
  loading: boolean;
  refreshing: boolean;
  error: string | null;
  credentials: Record<string, boolean>;
  quota: CodexQuota | null;
  quotaError: string | null;
  claudeQuota: ClaudeQuota | null;
  claudeQuotaError: string | null;
};

export type UsageData = UsageSnapshot & {
  secrets: Record<string, string>;
  credentialMessage: string;
  refresh: () => Promise<void>;
  toggleSource: (sourceId: string, enabled: boolean) => Promise<void>;
  saveCredential: (provider: string) => Promise<void>;
  setSecret: (provider: string, secret: string) => void;
  clearHistory: () => Promise<void>;
};

const emptySnapshot: UsageSnapshot = {
  data: null,
  sourceSettings: {},
  loading: true,
  refreshing: false,
  error: null,
  credentials: {},
  quota: null,
  quotaError: null,
  claudeQuota: null,
  claudeQuotaError: null,
};

async function publishRefreshRequest() {
  if (getCurrentWindow().label === "compact") {
    await emitTo("large", "usage-data-refresh-requested").catch(() => undefined);
  }
}

export function useUsageData(): UsageData {
  const [snapshot, setSnapshot] = useState<UsageSnapshot>(emptySnapshot);
  const [secrets, setSecrets] = useState<Record<string, string>>({});
  const [credentialMessage, setCredentialMessage] = useState("");
  const isLargeWindow = getCurrentWindow().label === "large" || getCurrentWindow().label === "main";

  const load = useCallback(async (showRefreshing = false) => {
    if (showRefreshing) setSnapshot((current) => ({ ...current, refreshing: true }));
    const [dashboardResult, settingsResult, credentialsResult, codexResult, claudeResult] = await Promise.allSettled([
      getUsageDashboard(),
      getUsageSourceSettings(),
      getProviderCredentialStatus(),
      getCodexQuota(),
      getClaudeQuota(),
    ]);

    const next: UsageSnapshot = {
      data: dashboardResult.status === "fulfilled" ? dashboardResult.value : null,
      sourceSettings: settingsResult.status === "fulfilled" ? settingsResult.value : {},
      credentials: credentialsResult.status === "fulfilled" ? credentialsResult.value : {},
      error: dashboardResult.status === "rejected" ? "Não foi possível carregar os dados locais." : null,
      quota: codexResult.status === "fulfilled" ? codexResult.value : null,
      quotaError: codexResult.status === "rejected"
        ? typeof codexResult.reason === "string" ? codexResult.reason : "Não foi possível consultar os limites. Confira se o Codex está conectado à sua conta ChatGPT."
        : null,
      claudeQuota: claudeResult.status === "fulfilled" ? claudeResult.value : null,
      claudeQuotaError: claudeResult.status === "rejected"
        ? typeof claudeResult.reason === "string" ? claudeResult.reason : "Não foi possível consultar os limites. Confira se o Claude Code está conectado à sua conta Claude."
        : null,
      loading: false,
      refreshing: false,
    };
    setSnapshot(next);
    if (isLargeWindow) await emitTo("compact", "usage-snapshot-updated", next).catch(() => undefined);
  }, [isLargeWindow]);

  useEffect(() => {
    let disposed = false;
    let unlistenSnapshot: (() => void) | undefined;
    let unlistenRefreshRequest: (() => void) | undefined;

    if (isLargeWindow) {
      const timer = window.setInterval(() => void load(), 60_000);
      void listen<void>("usage-data-refresh-requested", () => {
        void load();
      }).then((unlisten) => {
        if (disposed) unlisten();
        else unlistenRefreshRequest = unlisten;
      });
      void refreshUsageSources().then(() => load()).catch(() => load());
      return () => {
        disposed = true;
        window.clearInterval(timer);
        unlistenRefreshRequest?.();
      };
    }

    void listen<UsageSnapshot>("usage-snapshot-updated", (event) => setSnapshot(event.payload)).then((unlisten) => {
      if (disposed) unlisten();
      else {
        unlistenSnapshot = unlisten;
        void load();
      }
    });
    return () => {
      disposed = true;
      unlistenSnapshot?.();
    };
  }, [isLargeWindow, load]);

  const refresh = useCallback(async () => {
    setSnapshot((current) => ({ ...current, refreshing: true }));
    try {
      await refreshUsageSources();
    } finally {
      await load(true);
      await publishRefreshRequest();
    }
  }, [load]);

  const toggleSource = useCallback(async (sourceId: string, enabled: boolean) => {
    await setUsageSourceEnabled(sourceId, enabled);
    if (enabled) await refreshUsageSources();
    await load();
    await publishRefreshRequest();
  }, [load]);

  const saveCredential = useCallback(async (provider: string) => {
    setCredentialMessage("");
    try {
      const secret = secrets[provider]?.trim();
      if (secret) await saveProviderCredential(provider, secret);
      else if (snapshot.credentials[provider]) await deleteProviderCredential(provider);
      setSecrets((current) => ({ ...current, [provider]: "" }));
      await load();
      await publishRefreshRequest();
      setCredentialMessage("Chave atualizada com segurança.");
    } catch {
      setCredentialMessage("Não foi possível atualizar a chave.");
    }
  }, [load, secrets, snapshot.credentials]);

  const setSecret = useCallback((provider: string, secret: string) => {
    setSecrets((current) => ({ ...current, [provider]: secret }));
  }, []);

  const clearHistory = useCallback(async () => {
    await clearUsageHistory();
    await load();
    await publishRefreshRequest();
  }, [load]);

  return {
    ...snapshot,
    secrets,
    credentialMessage,
    refresh,
    toggleSource,
    saveCredential,
    setSecret,
    clearHistory,
  };
}
