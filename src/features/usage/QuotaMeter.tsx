import type { CSSProperties } from "react";
import type { CodexQuotaWindow } from "./types";

export default function QuotaMeter({
  title,
  quota,
  compact = false,
  loading = false,
}: {
  title: string;
  quota: CodexQuotaWindow | null;
  compact?: boolean;
  loading?: boolean;
}) {
  const remaining = quota ? Math.max(0, Math.min(100, 100 - quota.usedPercent)) : null;
  const reset = quota
    ? new Date(quota.resetsAt * 1000).toLocaleString("pt-BR", { weekday: "short", hour: "2-digit", minute: "2-digit" })
    : null;

  if (compact) {
    return (
      <article className={`compact-quota ${quota ? "" : "is-unavailable"}`}>
        <div
          className="quota-ring"
          role="progressbar"
          aria-label={`${title}: ${remaining === null ? "indisponível" : `${Math.round(remaining)}% restante`}`}
          aria-valuemin={0}
          aria-valuemax={100}
          aria-valuenow={remaining === null ? undefined : Math.round(remaining)}
          style={remaining === null ? undefined : { "--quota-remaining": `${remaining}%` } as CSSProperties}
        >
          <strong>{remaining === null ? "—" : `${Math.round(remaining)}%`}</strong>
        </div>
        <strong className="compact-quota-title">{title}</strong>
        <small>{reset ? `Renova ${reset}` : loading ? "Consultando…" : "Limite indisponível"}</small>
      </article>
    );
  }

  return (
    <div className={`quota-meter ${quota ? "" : "is-unavailable"}`}>
      <div className="quota-meter-heading">
        <strong>{title}</strong>
        <span>{remaining === null ? "Indisponível" : `${Math.round(remaining)}% restante`}</span>
      </div>
      <div className="quota-track" role="progressbar" aria-label={`${title}: ${remaining === null ? "indisponível" : `${Math.round(remaining)}% restante`}`} aria-valuemin={0} aria-valuemax={100} aria-valuenow={remaining === null ? undefined : Math.round(remaining)}>
        {remaining !== null && <div className="quota-fill" style={{ width: `${remaining}%` }} />}
      </div>
      <small>{reset ? `Renova ${reset}` : loading ? "Consultando limite…" : "Limite indisponível"}</small>
    </div>
  );
}
