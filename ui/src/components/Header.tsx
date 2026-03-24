import type { NormalizedSession } from "../types";
import { CrowIcon } from "./CrowIcon";

interface HeaderProps {
  sessions: NormalizedSession[];
  connected: boolean;
}

export function Header({ sessions, connected }: HeaderProps) {
  const working = sessions.filter((s) => s.status === "Working").length;
  const waiting = sessions.filter((s) => s.status === "WaitingInput").length;
  const idle = sessions.filter((s) => s.status === "Idle").length;

  return (
    <header className="header">
      <div className="header__left">
        <CrowIcon size={20} className="header__icon" />
        <span className="header__wordmark">caw</span>
        <span className="header__tagline">coding assistant watcher</span>
      </div>
      <div className="header__right">
        {working > 0 && (
          <span className="header__badge header__badge--working">
            ● {working} working
          </span>
        )}
        {waiting > 0 && (
          <span className="header__badge header__badge--waiting">
            ▲ {waiting} waiting
          </span>
        )}
        {idle > 0 && (
          <span className="header__badge header__badge--idle">
            ◉ {idle} idle
          </span>
        )}
        <span
          className={`header__connection ${connected ? "header__connection--on" : "header__connection--off"}`}
          title={connected ? "Connected" : "Reconnecting..."}
        />
      </div>
    </header>
  );
}
