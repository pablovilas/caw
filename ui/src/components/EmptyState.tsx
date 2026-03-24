import { CrowIcon } from "./CrowIcon";

export function EmptyState() {
  return (
    <div className="empty-state">
      <CrowIcon size={48} className="empty-state__icon" />
      <p className="empty-state__title">No coding assistants detected</p>
      <p className="empty-state__hint">
        Start a Claude Code, Codex, or OpenCode session
      </p>
    </div>
  );
}
