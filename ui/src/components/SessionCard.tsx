import type { NormalizedSession } from "../types";
import { STATUS_COLORS, STATUS_SYMBOLS, STATUS_LABELS, totalTokens, formatTokens } from "../types";

interface SessionCardProps {
  session: NormalizedSession;
}

export function SessionCard({ session }: SessionCardProps) {
  const total = totalTokens(session.token_usage);
  const cost = session.model ? estimateCost(session.token_usage, session.model) : null;

  return (
    <div className="session-card">
      <div className="session-card-header">
        <span className="plugin-badge">{session.display_name}</span>
        <span
          className="status-badge"
          style={{ backgroundColor: STATUS_COLORS[session.status] + "22", color: STATUS_COLORS[session.status], borderColor: STATUS_COLORS[session.status] + "44" }}
        >
          {STATUS_SYMBOLS[session.status]} {STATUS_LABELS[session.status]}
        </span>
      </div>

      <div className="session-card-body">
        <div className="project-name">{session.project_name}</div>
        {session.git_branch && <span className="branch-badge">⎇ {session.git_branch}</span>}
        {session.last_message && (
          <p className="last-message">{session.last_message.slice(0, 200)}</p>
        )}
      </div>

      <div className="session-card-footer">
        <span className="token-count">{formatTokens(total)} tokens</span>
        {cost !== null && <span className="cost">${cost.toFixed(4)}</span>}
        {session.model && <span className="model-tag">{session.model}</span>}
        {session.pid && <span className="pid-tag">PID {session.pid}</span>}
      </div>
    </div>
  );
}

function estimateCost(usage: { input: number; output: number; cache_read: number; cache_write: number }, model: string): number {
  const perM = 1_000_000;
  let ip = 3, op = 15, crp = 0.3, cwp = 3.75; // sonnet defaults

  if (model.includes("opus")) { ip = 15; op = 75; crp = 1.5; cwp = 18.75; }
  else if (model.includes("haiku")) { ip = 0.8; op = 4; crp = 0.08; cwp = 1; }
  else if (model.includes("gpt-4o")) { ip = 2.5; op = 10; crp = 1.25; cwp = 2.5; }

  return (usage.input * ip + usage.output * op + usage.cache_read * crp + usage.cache_write * cwp) / perM;
}
