import { useMemo } from "react";
import type { NormalizedSession, SessionStatus } from "../types";

export interface ProjectGroup {
  projectPath: string;
  projectName: string;
  gitBranch: string | null;
  sessions: NormalizedSession[];
}

const STATUS_ORDER: Record<SessionStatus, number> = {
  Working: 0,
  WaitingInput: 1,
  Idle: 2,
  Dead: 3,
};

export function useGroupedSessions(
  sessions: NormalizedSession[],
  activePlugins: Set<string>,
): ProjectGroup[] {
  return useMemo(() => {
    // Filter by plugin
    let filtered = sessions;
    if (activePlugins.size > 0) {
      filtered = sessions.filter((s) => activePlugins.has(s.display_name));
    }

    // Group by project_path
    const groupMap = new Map<string, NormalizedSession[]>();
    for (const session of filtered) {
      const key = session.project_path;
      if (!groupMap.has(key)) groupMap.set(key, []);
      groupMap.get(key)!.push(session);
    }

    // Build groups, sort sessions within each
    const groups: ProjectGroup[] = [];
    for (const [projectPath, groupSessions] of groupMap) {
      groupSessions.sort(
        (a, b) => STATUS_ORDER[a.status] - STATUS_ORDER[b.status],
      );
      const first = groupSessions[0];
      groups.push({
        projectPath,
        projectName: first.project_name,
        gitBranch: first.git_branch,
        sessions: groupSessions,
      });
    }

    // Sort groups by best status
    groups.sort((a, b) => {
      const bestA = Math.min(...a.sessions.map((s) => STATUS_ORDER[s.status]));
      const bestB = Math.min(...b.sessions.map((s) => STATUS_ORDER[s.status]));
      return bestA - bestB || a.projectPath.localeCompare(b.projectPath);
    });

    return groups;
  }, [sessions, activePlugins]);
}
