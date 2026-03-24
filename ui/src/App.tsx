import { useState, useMemo } from "react";
import { useSessions } from "./hooks/useWebSocket";
import { useGroupedSessions } from "./hooks/useGroupedSessions";
import { Header } from "./components/Header";
import { FilterBar } from "./components/FilterBar";
import { SessionTable } from "./components/SessionTable";
import { EmptyState } from "./components/EmptyState";

function App() {
  const { sessions, connected } = useSessions();
  const [activePlugins, setActivePlugins] = useState<Set<string>>(new Set());

  const plugins = useMemo(
    () => [...new Set(sessions.map((s) => s.display_name))],
    [sessions],
  );

  const groups = useGroupedSessions(sessions, activePlugins);

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
      <Header sessions={sessions} connected={connected} />
      <FilterBar
        plugins={plugins}
        activePlugins={activePlugins}
        onToggle={togglePlugin}
      />
      {groups.length === 0 ? (
        <EmptyState />
      ) : (
        <SessionTable groups={groups} />
      )}
    </div>
  );
}

export default App;
