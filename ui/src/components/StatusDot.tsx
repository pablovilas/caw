import type { SessionStatus } from "../types";
import { STATUS_COLORS } from "../types";

interface StatusDotProps {
  status: SessionStatus;
}

export function StatusDot({ status }: StatusDotProps) {
  if (status === "Dead") {
    return <span className="status-dot status-dot--dead">✕</span>;
  }

  return (
    <span
      className={`status-dot status-dot--${status.toLowerCase()}`}
      style={{ backgroundColor: STATUS_COLORS[status] }}
    />
  );
}
