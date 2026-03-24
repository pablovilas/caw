export type SessionStatus = "Working" | "WaitingInput" | "Idle" | "Dead";

export interface TokenUsage {
  input: number;
  output: number;
  cache_read: number;
  cache_write: number;
}

export interface NormalizedSession {
  id: string;
  plugin: string;
  display_name: string;
  project_path: string;
  project_name: string;
  status: SessionStatus;
  last_message: string | null;
  git_branch: string | null;
  model: string | null;
  token_usage: TokenUsage;
  started_at: string;
  last_seen: string;
  pid: number | null;
  app_name: string | null;
}

export type MonitorEvent =
  | { Added: NormalizedSession }
  | { Updated: NormalizedSession }
  | { Removed: { id: string; plugin: string } }
  | { Snapshot: NormalizedSession[] };

export const STATUS_COLORS: Record<SessionStatus, string> = {
  Working: "#1D9E75",
  WaitingInput: "#EF9F27",
  Idle: "#888780",
  Dead: "#E24B4A",
};

export const STATUS_SYMBOLS: Record<SessionStatus, string> = {
  Working: "●",
  WaitingInput: "▲",
  Idle: "◉",
  Dead: "✕",
};

export const STATUS_LABELS: Record<SessionStatus, string> = {
  Working: "working",
  WaitingInput: "waiting",
  Idle: "idle",
  Dead: "dead",
};
