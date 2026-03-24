import type { NormalizedSession } from "../types";
import { SessionRow } from "./SessionRow";

interface ProjectGroupProps {
  projectName: string;
  gitBranch: string | null;
  sessions: NormalizedSession[];
}

export function ProjectGroup({ projectName, gitBranch, sessions }: ProjectGroupProps) {
  return (
    <div className="project-group">
      <div className="project-group__header">
        <span className="project-group__rule" />
        <span className="project-group__name">{projectName}</span>
        {gitBranch && (
          <span className="project-group__branch">@{gitBranch}</span>
        )}
        <span className="project-group__rule project-group__rule--fill" />
      </div>
      <div className="project-group__rows">
        {sessions.map((session) => (
          <SessionRow key={session.id} session={session} />
        ))}
      </div>
    </div>
  );
}
