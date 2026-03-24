import type { NormalizedSession } from "../types";
import { STATUS_LABELS } from "../types";
import { StatusDot } from "./StatusDot";
import { totalTokens, formatTokens, estimateCost, formatCost } from "../lib/format";

interface SessionRowProps {
  session: NormalizedSession;
}

export function SessionRow({ session }: SessionRowProps) {
  const total = totalTokens(session.token_usage);
  const cost =
    session.model && total > 0
      ? estimateCost(session.token_usage, session.model)
      : 0;

  const rowClass = [
    "session-row",
    session.status === "Idle" && "session-row--idle",
    session.status === "Dead" && "session-row--dead",
  ]
    .filter(Boolean)
    .join(" ");

  return (
    <div className={rowClass}>
      <div className="session-row__status">
        <StatusDot status={session.status} />
        <span
          className="session-row__status-label"
          style={{ color: `var(--color-${session.status.toLowerCase()})` }}
        >
          {STATUS_LABELS[session.status]}
        </span>
      </div>
      <div className="session-row__assistant">{session.display_name}</div>
      <div className="session-row__app">{session.app_name || "-"}</div>
      <div className="session-row__message">
        {session.last_message?.replace(/\n/g, " ") || ""}
      </div>
      <div className="session-row__tokens">{formatTokens(total)}</div>
      <div className="session-row__cost">{formatCost(cost)}</div>
    </div>
  );
}
