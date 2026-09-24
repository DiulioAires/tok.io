import type { UsageData } from "./usageData";
import QuotaMeter from "./QuotaMeter";
import WidgetHeader, { type WidgetMode } from "./WidgetHeader";
import ProviderIcon from "./ProviderIcon";

export default function LargeUsageWidget({
  usage,
  mode,
  onChangeMode,
  onOpenSettings,
  onMenuOpenChange,
}: {
  usage: UsageData;
  mode: WidgetMode;
  onChangeMode: (mode: WidgetMode) => void;
  onOpenSettings: () => void;
  onMenuOpenChange: (open: boolean) => void;
}) {
  return (
    <section className="usage-dashboard large-widget" aria-live="polite">
      <WidgetHeader mode={mode} onChangeMode={onChangeMode} onOpenSettings={onOpenSettings} onMenuOpenChange={onMenuOpenChange} />
      <div className="large-widget-cards">
        <article className="quota-card quota-card-openai">
          <div className="quota-card-heading">
            <ProviderIcon provider="openai" />
            <div className="quota-card-title"><span className="usage-eyebrow">{usage.quota?.planType?.toUpperCase() ?? "CHATGPT WORK"}</span><strong>ChatGPT Work + Codex</strong></div>
            {usage.quota?.refreshedAt && <small>Atualizado {new Date(usage.quota.refreshedAt * 1000).toLocaleTimeString("pt-BR", { hour: "2-digit", minute: "2-digit" })}</small>}
          </div>
          <QuotaMeter title="Janela de 5 horas" quota={usage.quota?.fiveHour ?? null} loading={usage.loading} />
          <small className="weekly-remaining">{usage.quota?.weekly ? `${Math.max(0, Math.min(100, Math.round(100 - usage.quota.weekly.usedPercent)))}%` : usage.loading ? "…" : "—"}</small>
          <small className="quota-shared-note">O saldo é compartilhado entre Work e Codex.</small>
          {usage.quotaError && <p className="quota-error-message">{usage.quotaError}</p>}
        </article>

        <article className="quota-card quota-card-claude">
          <div className="quota-card-heading">
            <ProviderIcon provider="claude" />
            <div className="quota-card-title"><span className="usage-eyebrow">ASSINATURA CLAUDE</span><strong>Claude Code</strong></div>
            {usage.claudeQuota?.refreshedAt && <small>Atualizado {new Date(usage.claudeQuota.refreshedAt * 1000).toLocaleTimeString("pt-BR", { hour: "2-digit", minute: "2-digit" })}</small>}
          </div>
          <QuotaMeter title="Janela de 5 horas" quota={usage.claudeQuota?.fiveHour ?? null} loading={usage.loading} />
          <small className="weekly-remaining">{usage.claudeQuota?.weekly ? `${Math.max(0, Math.min(100, Math.round(100 - usage.claudeQuota.weekly.usedPercent)))}%` : usage.loading ? "…" : "—"}</small>
          <small className="quota-shared-note">Limites lidos da sessão local do Claude Code.</small>
          {usage.claudeQuotaError && <p className="quota-error-message">{usage.claudeQuotaError}</p>}
        </article>
      </div>
      <footer className="widget-footer">
        <span className={usage.error ? "connection-dot is-warning" : "connection-dot"} />
        <span>{usage.error ?? (usage.refreshing ? "Atualizando…" : "Dados atualizados automaticamente")}</span>
        <button className="footer-refresh" onClick={() => void usage.refresh()} disabled={usage.refreshing} aria-label="Atualizar dados agora">
          <svg viewBox="0 0 24 24" aria-hidden="true"><path d="M20 7v5h-5M5.64 8a8 8 0 0 1 13.15-2.96L20 7M4 17v-5h5m9.36 4a8 8 0 0 1-13.15 2.96L4 17" /></svg>
        </button>
      </footer>
    </section>
  );
}
