import type { ProjectGroup } from "../hooks/useGroupedSessions";
import { ProjectGroup as ProjectGroupComponent } from "./ProjectGroup";

interface SessionTableProps {
  groups: ProjectGroup[];
}

export function SessionTable({ groups }: SessionTableProps) {
  return (
    <div className="session-table">
      <div className="session-table__header">
        <div className="session-row session-row--header">
          <div className="session-row__status">STATUS</div>
          <div className="session-row__assistant">ASSISTANT</div>
          <div className="session-row__app">APP</div>
          <div className="session-row__message">LAST MESSAGE</div>
          <div className="session-row__tokens">TOKENS</div>
          <div className="session-row__cost">COST</div>
        </div>
      </div>
      <div className="session-table__body">
        {groups.map((group) => (
          <ProjectGroupComponent
            key={group.projectPath}
            projectName={group.projectName}
            gitBranch={group.gitBranch}
            sessions={group.sessions}
          />
        ))}
      </div>
    </div>
  );
}
