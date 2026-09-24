import { useMemo } from "react";
import type { SourceStatus, UsageBucket } from "./types";
import type { UsageData } from "./usageData";

const sourceLabels: Record<string, string> = {
  codex_local: "Codex",
  claude_code_local: "Claude Code",
  openai_api: "OpenAI API",
  anthropic_api: "Anthropic API",
};

const stateLabels: Record<SourceStatus["state"], string> = {
  connected: "Conectado",
  partial: "Parcial",
  stale: "Desatualizado",
  unavailable: "Indisponível",
};

function formatTokens(value: number) {
  return new Intl.NumberFormat("pt-BR", { notation: "compact", maximumFractionDigits: 1 }).format(value);
}

function startOfToday() {
  const today = new Date();
  today.setHours(0, 0, 0, 0);
  return Math.floor(today.getTime() / 1000);
}

function dailyTotals(buckets: UsageBucket[]) {
  return Array.from({ length: 7 }, (_, index) => {
    const day = new Date();
    day.setHours(0, 0, 0, 0);
    day.setDate(day.getDate() - (6 - index));
    const start = Math.floor(day.getTime() / 1000);
    const end = start + 24 * 60 * 60;
    return {
      key: day.toISOString().slice(0, 10),
      label: day.toLocaleDateString("pt-BR", { weekday: "short" }).replace(".", ""),
      value: buckets.filter((bucket) => bucket.periodStart >= start && bucket.periodStart < end)
        .reduce((total, bucket) => total + (bucket.totalTokens ?? 0), 0),
    };
  });
}

function SourceCard({ source, enabled, onToggle }: {
  source: SourceStatus;
  enabled: boolean;
  onToggle: (sourceId: string, enabled: boolean) => void;
}) {
  return (
    <article className={`settings-source settings-source-${source.state}`}>
      <div className="settings-source-heading"><strong>{sourceLabels[source.sourceId] ?? source.sourceId}</strong><span>{stateLabels[source.state]}</span></div>
      {source.message && <p>{source.message}</p>}
      {source.lastSuccessfulRefresh && <small>Atualizado às {new Date(source.lastSuccessfulRefresh * 1000).toLocaleTimeString("pt-BR", { hour: "2-digit", minute: "2-digit" })}</small>}
      <button onClick={() => void onToggle(source.sourceId, !enabled)}>{enabled ? "Pausar" : "Ativar"} fonte</button>
    </article>
  );
}

export default function WidgetSettings({ usage }: { usage: UsageData }) {
  const buckets = usage.data?.buckets ?? [];
  const today = startOfToday();
  const todayBuckets = buckets.filter((bucket) => bucket.periodStart >= today);
  const todayTokens = todayBuckets.reduce((total, bucket) => total + (bucket.totalTokens ?? 0), 0);
  const todaySessions = new Set(todayBuckets.map((bucket) => bucket.sessionKey ?? bucket.id)).size;
  const days = useMemo(() => dailyTotals(buckets), [buckets]);
  const maxValue = Math.max(...days.map((day) => day.value), 1);
  const apiCosts = buckets.filter((bucket) => bucket.usageKind === "api_cost" && bucket.amount !== null)
    .reduce((totals, bucket) => {
      const currency = bucket.currency ?? "USD";
      totals[currency] = (totals[currency] ?? 0) + (bucket.amount ?? 0);
      return totals;
    }, {} as Record<string, number>);

  return (
    <section className="widget-settings" aria-label="Configurações e atividade">
      <section className="settings-section">
        <div className="settings-section-heading"><div><span className="usage-eyebrow">CONEXÕES</span><h2>Fontes de uso</h2></div><button onClick={() => void usage.refresh()} disabled={usage.refreshing}>{usage.refreshing ? "Atualizando…" : "Atualizar"}</button></div>
        {usage.error && <p className="usage-error">{usage.error}</p>}
        {usage.data?.sources.filter((source) => source.sourceId !== "chatgpt_work").map((source) => (
          <SourceCard key={source.sourceId} source={source} enabled={usage.sourceSettings[source.sourceId] !== false} onToggle={usage.toggleSource} />
        ))}
        {!usage.data && !usage.loading && <p className="usage-error">Nenhuma fonte conectada.</p>}
      </section>

      <section className="settings-section">
        <span className="usage-eyebrow">RELATÓRIOS DE API</span>
        <h2>Chaves administrativas</h2>
        <p>As chaves ficam no Gerenciador de Credenciais do Windows. O acesso administrativo é necessário para relatórios de custo e uso.</p>
        {([ ["openai", "OpenAI"], ["anthropic", "Anthropic"] ] as const).map(([provider, label]) => (
          <div className="settings-credential" key={provider}>
            <label htmlFor={`key-${provider}`}>{label}{usage.credentials[provider] ? " · chave salva" : ""}</label>
            <input id={`key-${provider}`} type="password" autoComplete="off" value={usage.secrets[provider] ?? ""} placeholder={usage.credentials[provider] ? "Substituir chave" : "Chave administrativa"} onChange={(event) => usage.setSecret(provider, event.target.value)} />
            <button disabled={!usage.secrets[provider] && !usage.credentials[provider]} onClick={() => void usage.saveCredential(provider)}>{usage.secrets[provider] ? "Salvar" : "Remover"}</button>
          </div>
        ))}
        {usage.credentialMessage && <small>{usage.credentialMessage}</small>}
      </section>

      <section className="settings-section settings-activity">
        <div className="settings-section-heading"><div><span className="usage-eyebrow">ATIVIDADE LOCAL</span><h2>Resumo de hoje</h2></div><button className="danger-button" onClick={() => void usage.clearHistory()}>Limpar histórico</button></div>
        <div className="settings-summary"><div><strong>{usage.loading ? "—" : formatTokens(todayTokens)}</strong><span>tokens observados hoje</span></div><div><strong>{usage.loading ? "—" : todaySessions}</strong><span>sessões</span></div></div>
        <div className="settings-api-costs"><span>Gastos de API nos últimos 7 dias</span>{Object.keys(apiCosts).length ? Object.entries(apiCosts).map(([currency, amount]) => <strong key={currency}>{new Intl.NumberFormat("pt-BR", { style: "currency", currency }).format(amount)}</strong>) : <small>Sem custos registrados. Assinaturas não entram nesta conta.</small>}</div>
        <div className="settings-chart" role="img" aria-label="Tokens observados por dia nos últimos sete dias">
          {days.map((day) => <div className="settings-chart-column" key={day.key} title={`${day.label}: ${formatTokens(day.value)} tokens`}><div className="settings-bar-track"><div className="settings-bar" style={{ height: `${day.value ? Math.max(8, (day.value / maxValue) * 100) : 0}%` }} /></div><span>{day.label}</span></div>)}
        </div>
      </section>
      <p className="settings-footnote">Tokens locais indicam atividade observada; o widget não salva prompts. Limites de assinatura vêm das sessões locais de ChatGPT/Codex e Claude Code.</p>
    </section>
  );
}
