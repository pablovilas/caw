import { useState, useMemo } from "react";
import { useSessions } from "./hooks/useWebSocket";
import { StatusBar } from "./components/StatusBar";
import { SessionCard } from "./components/SessionCard";
import { PluginFilter } from "./components/PluginFilter";
import type { SessionStatus } from "./types";

const STATUS_ORDER: Record<SessionStatus, number> = {
  Working: 0,
  WaitingInput: 1,
  Idle: 2,
  Dead: 3,
};

function App() {
  const { sessions, connected } = useSessions();
  const [activePlugins, setActivePlugins] = useState<Set<string>>(new Set());

  const plugins = useMemo(
    () => [...new Set(sessions.map((s) => s.display_name))],
    [sessions],
  );

  const filtered = useMemo(() => {
    let result = sessions;
    if (activePlugins.size > 0) {
      result = result.filter((s) => activePlugins.has(s.display_name));
    }
    return result.sort((a, b) => STATUS_ORDER[a.status] - STATUS_ORDER[b.status]);
  }, [sessions, activePlugins]);

  const togglePlugin = (plugin: string) => {
    setActivePlugins((prev) => {
      const next = new Set(prev);
      if (next.has(plugin)) next.delete(plugin);
      else next.add(plugin);
      return next;
    });
  };

  return (
    <div className="app">
      <StatusBar sessions={sessions} connected={connected} />
      <PluginFilter plugins={plugins} activePlugins={activePlugins} onToggle={togglePlugin} />
      <main className="session-grid">
        {filtered.length === 0 ? (
          <div className="empty-state">
            <div className="empty-icon">&#x1F426;&#x200D;&#x2B1B;</div>
            <p>No coding assistants detected</p>
            <p className="empty-hint">Start a Claude Code, Codex, or OpenCode session</p>
          </div>
        ) : (
          filtered.map((session) => (
            <SessionCard key={`${session.plugin}:${session.id}`} session={session} />
          ))
        )}
      </main>
    </div>
  );
}

export default App;
