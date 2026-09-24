import type { UsageData } from "./usageData";
import QuotaMeter from "./QuotaMeter";
import WidgetHeader, { type WidgetMode } from "./WidgetHeader";
import ProviderIcon from "./ProviderIcon";

export default function CompactUsageWidget({
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
    <section className="usage-dashboard compact-widget" aria-live="polite">
      <WidgetHeader mode={mode} onChangeMode={onChangeMode} onOpenSettings={onOpenSettings} onMenuOpenChange={onMenuOpenChange} />
      <div className="compact-widget-grid">
        <div className="compact-provider compact-provider-openai">
          <span className="compact-provider-name"><ProviderIcon provider="openai" compact />ChatGPT Work + Codex</span>
          <QuotaMeter title="5 horas" quota={usage.quota?.fiveHour ?? null} compact loading={usage.loading} />
          <QuotaMeter title="Semanal" quota={usage.quota?.weekly ?? null} compact loading={usage.loading} />
        </div>
        <div className="compact-provider compact-provider-claude">
          <span className="compact-provider-name"><ProviderIcon provider="claude" compact />Claude Code</span>
          <QuotaMeter title="5 horas" quota={usage.claudeQuota?.fiveHour ?? null} compact loading={usage.loading} />
          <QuotaMeter title="Semanal" quota={usage.claudeQuota?.weekly ?? null} compact loading={usage.loading} />
        </div>
      </div>
      <footer className="widget-footer compact-footer">
        <span className={usage.error ? "connection-dot is-warning" : "connection-dot"} />
        <span>{usage.error ?? (usage.refreshing ? "Atualizando…" : "Limites da conta")}</span>
        <button className="footer-refresh" onClick={() => void usage.refresh()} disabled={usage.refreshing} aria-label="Atualizar dados agora">
          <svg viewBox="0 0 24 24" aria-hidden="true"><path d="M20 7v5h-5M5.64 8a8 8 0 0 1 13.15-2.96L20 7M4 17v-5h5m9.36 4a8 8 0 0 1-13.15 2.96L4 17" /></svg>
        </button>
      </footer>
    </section>
  );
}
