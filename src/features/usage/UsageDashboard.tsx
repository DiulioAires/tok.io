import { useMemo } from "react";
import type { SourceStatus, UsageBucket } from "./types";
import { useUsageData } from "./usageData";
import "./UsageDashboard.css";

const SOURCE_LABELS: Record<string, string> = {
  codex_local: "Codex",
  claude_code_local: "Claude Code",
  openai_api: "OpenAI API",
  anthropic_api: "Anthropic API",
};

const STATE_LABELS: Record<SourceStatus["state"], string> = {
  connected: "Conectado",
  partial: "Parcial",
  stale: "Desatualizado",
  unavailable: "Indisponível",
};

function formatTokens(value: number) {
  return new Intl.NumberFormat("pt-BR", { notation: "compact", maximumFractionDigits: 1 }).format(value);
}

function QuotaMeter({ title, usedPercent, resetsAt }: { title: string; usedPercent: number; resetsAt: number }) {
  const remaining = Math.max(0, 100 - usedPercent);
  const reset = new Date(resetsAt * 1000).toLocaleString("pt-BR", {
    weekday: "short",
    hour: "2-digit",
    minute: "2-digit",
  });
  return (
    <div className="quota-meter">
      <div className="quota-meter-heading">
        <strong>{title}</strong>
        <span>{Math.round(remaining)}% restante</span>
      </div>
      <div className="quota-track" role="progressbar" aria-label={`${title}: ${Math.round(remaining)}% restante`} aria-valuemin={0} aria-valuemax={100} aria-valuenow={Math.round(remaining)}>
        <div className="quota-fill" style={{ width: `${remaining}%` }} />
      </div>
      <small>Renova {reset}</small>
    </div>
  );
}

function startOfToday() {
  const today = new Date();
  today.setHours(0, 0, 0, 0);
  return Math.floor(today.getTime() / 1000);
}

function SourceCard({
  source,
  enabled,
  onToggle,
}: {
  source: SourceStatus;
  enabled: boolean;
  onToggle: (sourceId: string, enabled: boolean) => void;
}) {
  return (
    <article className={`usage-source usage-source-${source.state}`}>
      <div className="usage-source-heading">
        <strong>{SOURCE_LABELS[source.sourceId] ?? source.sourceId}</strong>
        <div className="usage-source-actions">
          <span className="usage-source-state">{STATE_LABELS[source.state]}</span>
          <button onClick={() => onToggle(source.sourceId, !enabled)}>
            {enabled ? "Pausar" : "Ativar"}
          </button>
        </div>
      </div>
      {source.message && <p>{source.message}</p>}
      {source.lastSuccessfulRefresh && (
        <small>
          Atualizado às {new Date(source.lastSuccessfulRefresh * 1000).toLocaleTimeString("pt-BR", { hour: "2-digit", minute: "2-digit" })}
        </small>
      )}
    </article>
  );
}

function dailyTotals(buckets: UsageBucket[]) {
  const days: { key: string; label: string; value: number }[] = [];
  for (let offset = 6; offset >= 0; offset -= 1) {
    const day = new Date();
    day.setHours(0, 0, 0, 0);
    day.setDate(day.getDate() - offset);
    const start = Math.floor(day.getTime() / 1000);
    const end = start + 24 * 60 * 60;
    const value = buckets
      .filter((bucket) => bucket.periodStart >= start && bucket.periodStart < end)
      .reduce((total, bucket) => total + (bucket.totalTokens ?? 0), 0);
    days.push({
      key: day.toISOString().slice(0, 10),
      label: day.toLocaleDateString("pt-BR", { weekday: "short" }).replace(".", ""),
      value,
    });
  }
  return days;
}

export default function UsageDashboard() {
  const usage = useUsageData();
  const { data, sourceSettings, loading, refreshing, error, credentials, secrets,
    credentialMessage, quota, quotaError, claudeQuota, claudeQuotaError } = usage;

  const buckets = data?.buckets ?? [];
  const today = startOfToday();
  const todayTokens = buckets
    .filter((bucket) => bucket.periodStart >= today)
    .reduce((total, bucket) => total + (bucket.totalTokens ?? 0), 0);
  const todaySessions = new Set(
    buckets
      .filter((bucket) => bucket.periodStart >= today)
      .map((bucket) => bucket.sessionKey ?? bucket.id),
  ).size;
  const days = useMemo(() => dailyTotals(buckets), [buckets]);
  const maxValue = Math.max(...days.map((day) => day.value), 1);

  const apiCosts = buckets
    .filter((bucket) => bucket.usageKind === "api_cost" && bucket.amount !== null)
    .reduce((totals, bucket) => {
      const currency = bucket.currency ?? "USD";
      totals[currency] = (totals[currency] ?? 0) + (bucket.amount ?? 0);
      return totals;
    }, {} as Record<string, number>);

  return (
    <section className="usage-dashboard" aria-live="polite">
      <header className="usage-heading">
        <div className="usage-brand">
          <div className="usage-brand-icon"><img src="/tokio-icon.png" alt="" /></div>
          <div>
            <h1>tok.io</h1>
            <span>Seu uso de IA</span>
          </div>
        </div>
        <button className="usage-refresh" onClick={() => void usage.refresh()} disabled={refreshing} aria-label="Atualizar dados">
          {refreshing ? "…" : "↻"}
        </button>
      </header>

      <section className="quota-card">
        <div className="quota-card-heading">
          <div>
            <span className="usage-eyebrow">ASSINATURA {quota?.planType?.toUpperCase() ?? "CHATGPT"}</span>
            <strong>ChatGPT Work + Codex</strong>
          </div>
          {quota?.refreshedAt && <small>Atualizado às {new Date(quota.refreshedAt * 1000).toLocaleTimeString("pt-BR", { hour: "2-digit", minute: "2-digit" })}</small>}
        </div>
        {quota ? (
          <div className="quota-meters">
            {quota.fiveHour && <QuotaMeter title="Janela de 5 horas" usedPercent={quota.fiveHour.usedPercent} resetsAt={quota.fiveHour.resetsAt} />}
            {quota.weekly && <QuotaMeter title="Limite semanal" usedPercent={quota.weekly.usedPercent} resetsAt={quota.weekly.resetsAt} />}
          </div>
        ) : <p className={quotaError ? "usage-error" : "quota-message"}>{quotaError ?? "Consultando o saldo da conta…"}</p>}
        <small className="quota-shared-note">O saldo é compartilhado entre Work e Codex.</small>
      </section>

      <section className="quota-card">
        <div className="quota-card-heading">
          <div>
            <span className="usage-eyebrow">ASSINATURA CLAUDE</span>
            <strong>Claude Pro / Max</strong>
          </div>
          {claudeQuota?.refreshedAt && <small>Atualizado às {new Date(claudeQuota.refreshedAt * 1000).toLocaleTimeString("pt-BR", { hour: "2-digit", minute: "2-digit" })}</small>}
        </div>
        {claudeQuota ? (
          <div className="quota-meters">
            {claudeQuota.fiveHour && <QuotaMeter title="Janela de 5 horas" usedPercent={claudeQuota.fiveHour.usedPercent} resetsAt={claudeQuota.fiveHour.resetsAt} />}
            {claudeQuota.weekly && <QuotaMeter title="Limite semanal" usedPercent={claudeQuota.weekly.usedPercent} resetsAt={claudeQuota.weekly.resetsAt} />}
          </div>
        ) : <p className={claudeQuotaError ? "usage-error" : "quota-message"}>{claudeQuotaError ?? "Consultando os limites da assinatura…"}</p>}
        <small className="quota-shared-note">Consulta local à sessão Claude Code. O endpoint de assinatura não é uma API pública documentada.</small>
      </section>

      <section className="usage-summary">
        <div>
          <span className="usage-eyebrow">TOKENS REGISTRADOS HOJE</span>
          <strong>{loading ? "—" : formatTokens(todayTokens)}</strong>
          <p>Observados nas sessões locais</p>
        </div>
        <div className="usage-session-count">
          <strong>{loading ? "—" : todaySessions}</strong>
          <span>sessões</span>
        </div>
      </section>

      <section className="usage-api-costs">
        <span className="usage-eyebrow">GASTOS DE API NOS ÚLTIMOS 7 DIAS</span>
        {Object.keys(apiCosts).length ? Object.entries(apiCosts).map(([currency, amount]) => (
          <strong key={currency}>{new Intl.NumberFormat("pt-BR", { style: "currency", currency }).format(amount)}</strong>
        )) : <p>Sem custos de API registrados. Assinaturas não entram nesta conta.</p>}
      </section>

      <section className="usage-credentials">
        <strong>Conectar relatórios de API</strong>
        <p>Use chaves administrativas de organização. Elas ficam no Gerenciador de Credenciais do Windows.</p>
        {([ ["openai", "OpenAI"], ["anthropic", "Anthropic"] ] as const).map(([provider, label]) => (
          <div className="usage-credential-row" key={provider}>
            <label htmlFor={`key-${provider}`}>{label}{credentials[provider] ? " · chave salva" : ""}</label>
            <input id={`key-${provider}`} type="password" autoComplete="off" value={secrets[provider] ?? ""} placeholder={credentials[provider] ? "Substituir chave" : "Chave administrativa"} onChange={(event) => usage.setSecret(provider, event.target.value)} />
            <button disabled={!secrets[provider] && !credentials[provider]} onClick={() => void usage.saveCredential(provider)}>{secrets[provider] ? "Salvar" : "Remover"}</button>
          </div>
        ))}
        {credentialMessage && <small>{credentialMessage}</small>}
      </section>

      <section className="usage-chart-card">
        <div className="usage-chart-header">
          <div><span>ATIVIDADE</span><strong>Últimos 7 dias</strong></div>
          <small>tokens observados</small>
        </div>
        <div className="usage-chart" role="img" aria-label="Tokens observados por dia nos últimos sete dias">
          {days.map((day) => (
            <div className="usage-chart-column" key={day.key} title={`${day.label}: ${formatTokens(day.value)} tokens`}>
              <div className="usage-bar-track">
                <div className="usage-bar" style={{ height: `${day.value ? Math.max(8, (day.value / maxValue) * 100) : 0}%` }} />
              </div>
              <span>{day.label}</span>
            </div>
          ))}
        </div>
      </section>

      <section className="usage-sources">
        <div className="usage-section-title">
          <strong>Fontes de uso</strong>
          <button onClick={() => void usage.clearHistory()} title="Apagar o histórico coletado pelo widget">Limpar histórico</button>
        </div>
        {error && <p className="usage-error">{error}</p>}
        {data?.sources.filter((source) => source.sourceId !== "chatgpt_work").map((source) => (
          <SourceCard
            source={source}
            enabled={sourceSettings[source.sourceId] !== false}
            onToggle={(sourceId, enabled) => void usage.toggleSource(sourceId, enabled)}
            key={source.sourceId}
          />
        ))}
        {!data && !loading && <p className="usage-error">{error ?? "Nenhuma fonte conectada."}</p>}
      </section>

      <p className="usage-footnote">Os saldos de assinatura vêm das sessões locais do ChatGPT/Codex e Claude Code. O limite do Claude usa um endpoint interno da Anthropic, que pode mudar. Tokens locais são apenas atividade observada; nenhum prompt é salvo pelo widget.</p>
    </section>
  );
}
