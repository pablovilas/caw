import { useEffect, useRef, useCallback, useState } from "react";
import type { NormalizedSession, MonitorEvent } from "../types";

const WS_URL = "ws://localhost:7272";
const POLL_URL = "http://localhost:7272/api/sessions";
const POLL_INTERVAL = 3000;
const RECONNECT_DELAY = 2000;

export function useSessions() {
  const [sessions, setSessions] = useState<NormalizedSession[]>([]);
  const [connected, setConnected] = useState(false);
  const wsRef = useRef<WebSocket | null>(null);
  const pollRef = useRef<ReturnType<typeof setInterval> | null>(null);

  const applyEvent = useCallback((event: MonitorEvent) => {
    if ("Snapshot" in event) {
      setSessions(event.Snapshot);
    } else if ("Added" in event) {
      setSessions((prev) => [...prev.filter((s) => s.id !== event.Added.id || s.plugin !== event.Added.plugin), event.Added]);
    } else if ("Updated" in event) {
      setSessions((prev) =>
        prev.map((s) =>
          s.id === event.Updated.id && s.plugin === event.Updated.plugin ? event.Updated : s,
        ),
      );
    } else if ("Removed" in event) {
      setSessions((prev) =>
        prev.filter((s) => !(s.id === event.Removed.id && s.plugin === event.Removed.plugin)),
      );
    }
  }, []);

  const startPolling = useCallback(() => {
    if (pollRef.current) return;
    pollRef.current = setInterval(async () => {
      try {
        const res = await fetch(POLL_URL);
        if (res.ok) {
          const data: NormalizedSession[] = await res.json();
          setSessions(data);
        }
      } catch {
        // ignore
      }
    }, POLL_INTERVAL);
  }, []);

  const stopPolling = useCallback(() => {
    if (pollRef.current) {
      clearInterval(pollRef.current);
      pollRef.current = null;
    }
  }, []);

  const connect = useCallback(() => {
    try {
      const ws = new WebSocket(WS_URL);

      ws.onopen = () => {
        setConnected(true);
        stopPolling();
      };

      ws.onmessage = (e) => {
        try {
          const event: MonitorEvent = JSON.parse(e.data);
          applyEvent(event);
        } catch {
          // ignore
        }
      };

      ws.onclose = () => {
        setConnected(false);
        startPolling();
        setTimeout(connect, RECONNECT_DELAY);
      };

      ws.onerror = () => {
        ws.close();
      };

      wsRef.current = ws;
    } catch {
      startPolling();
      setTimeout(connect, RECONNECT_DELAY);
    }
  }, [applyEvent, startPolling, stopPolling]);

  useEffect(() => {
    connect();
    return () => {
      wsRef.current?.close();
      stopPolling();
    };
  }, [connect, stopPolling]);

  return { sessions, connected };
}
