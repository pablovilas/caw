import type { NormalizedSession } from "../types";
import { STATUS_COLORS } from "../types";

interface StatusBarProps {
  sessions: NormalizedSession[];
  connected: boolean;
}

export function StatusBar({ sessions, connected }: StatusBarProps) {
  const counts = {
    Working: sessions.filter((s) => s.status === "Working").length,
    WaitingInput: sessions.filter((s) => s.status === "WaitingInput").length,
    Idle: sessions.filter((s) => s.status === "Idle").length,
    Dead: sessions.filter((s) => s.status === "Dead").length,
  };

  return (
    <header className="status-bar">
      <div className="status-bar-left">
        <h1 className="wordmark">caw</h1>
        <span className="tagline">coding assistant watcher</span>
      </div>
      <div className="status-bar-right">
        {counts.Working > 0 && (
          <span className="status-count" style={{ color: STATUS_COLORS.Working }}>
            {counts.Working} working
          </span>
        )}
        {counts.WaitingInput > 0 && (
          <span className="status-count" style={{ color: STATUS_COLORS.WaitingInput }}>
            {counts.WaitingInput} waiting
          </span>
        )}
        {counts.Idle > 0 && (
          <span className="status-count" style={{ color: STATUS_COLORS.Idle }}>
            {counts.Idle} idle
          </span>
        )}
        <span className={`connection-dot ${connected ? "connected" : "disconnected"}`} title={connected ? "Connected" : "Reconnecting..."} />
      </div>
    </header>
  );
}
