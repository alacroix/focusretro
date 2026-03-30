import { listen } from "@tauri-apps/api/event";
import { useEffect, useState } from "react";

import {
  TraceEntry,
  getTraces,
  clearTraces,
  getListenerHealth,
  ListenerHealth,
} from "../lib/commands";

function latencyColor(ms: number): string {
  if (ms < 50) return "text-emerald-600 dark:text-emerald-400";
  if (ms < 150) return "text-amber-600 dark:text-amber-400";
  return "text-red-600 dark:text-red-400";
}

function formatTime(ms: number): string {
  const d = new Date(ms);
  return d.toLocaleTimeString("fr-FR", {
    hour: "2-digit",
    minute: "2-digit",
    second: "2-digit",
  });
}

function eventLabel(type: string): string {
  if (type === "turn") return "Turn";
  if (type === "group_invite") return "Group invite";
  if (type === "trade") return "Trade";
  if (type === "notification_center_restart") return "NC restarted";
  if (type === "listener_reconnect") return "Reconnected";
  return type;
}

function eventBadge(type: string): string {
  if (type === "turn")
    return "bg-brand-50 text-brand-700 border border-brand-200 dark:bg-brand-900/60 dark:text-brand-300 dark:border-brand-700/50";
  if (type === "group_invite")
    return "bg-emerald-50 text-emerald-700 border border-emerald-200 dark:bg-emerald-900/60 dark:text-emerald-300 dark:border-emerald-700/50";
  if (type === "trade")
    return "bg-amber-50 text-amber-700 border border-amber-200 dark:bg-amber-900/60 dark:text-amber-300 dark:border-amber-700/50";
  if (type === "notification_center_restart")
    return "bg-red-50 text-red-700 border border-red-200 dark:bg-red-900/60 dark:text-red-300 dark:border-red-700/50";
  if (type === "listener_reconnect")
    return "bg-emerald-50 text-emerald-700 border border-emerald-200 dark:bg-emerald-900/60 dark:text-emerald-300 dark:border-emerald-700/50";
  return "bg-gray-100 text-gray-700 dark:bg-gray-800 dark:text-gray-300";
}

export default function DebugPanel() {
  const [traces, setTraces] = useState<TraceEntry[]>([]);
  const [health, setHealth] = useState<ListenerHealth>({
    healthy: false,
    restart_count: 0,
    mode: "unknown",
  });
  const reload = () => getTraces().then(setTraces);
  const reloadHealth = () => getListenerHealth().then(setHealth);

  useEffect(() => {
    reload();
    reloadHealth();
    const unlistenTrace = listen("trace-added", () => {
      reload();
      reloadHealth();
    });
    const unlistenMode = listen<string>("notif-mode-changed", () => reloadHealth());
    return () => {
      unlistenTrace.then((f) => f());
      unlistenMode.then((f) => f());
    };
  }, []);

  const handleClear = () => clearTraces().then(reload);

  const reversed = [...traces].reverse();

  return (
    <div className="flex flex-col gap-4">
      <div className="flex items-center justify-between">
        <div className="flex items-center gap-2">
          <h2 className="text-xs font-semibold tracking-wider text-gray-500 uppercase dark:text-gray-400">
            Traces
          </h2>
          <span
            className={`rounded border px-1.5 py-0.5 text-[10px] font-medium ${
              health.mode === "event"
                ? "border-emerald-200 bg-emerald-50 text-emerald-700 dark:border-emerald-700/50 dark:bg-emerald-900/60 dark:text-emerald-300"
                : health.mode === "poll"
                  ? "border-amber-200 bg-amber-50 text-amber-700 dark:border-amber-700/50 dark:bg-amber-900/60 dark:text-amber-300"
                  : health.mode === "poll-db"
                    ? "border-amber-200 bg-amber-50 text-amber-700 dark:border-amber-700/50 dark:bg-amber-900/60 dark:text-amber-300"
                    : "border-gray-200 bg-gray-100 text-gray-500 dark:border-gray-700 dark:bg-gray-800 dark:text-gray-500"
            }`}
          >
            {health.mode === "event"
              ? "event-driven"
              : health.mode === "poll"
                ? "polling 100ms"
                : health.mode === "poll-db"
                  ? "polling DB 200ms"
                  : health.mode}
          </span>
          {health.restart_count > 0 && (
            <span className="rounded border border-red-200 bg-red-50 px-1.5 py-0.5 text-[10px] font-medium text-red-600 dark:border-red-700/50 dark:bg-red-900/60 dark:text-red-300">
              {health.restart_count} restart{health.restart_count > 1 ? "s" : ""}
            </span>
          )}
        </div>
        <button
          onClick={handleClear}
          className="cursor-pointer rounded border border-gray-200 bg-gray-100 px-2 py-1 text-xs text-gray-700 hover:bg-gray-200 dark:border-gray-700 dark:bg-gray-800 dark:text-gray-300 dark:hover:bg-gray-700"
        >
          Clear
        </button>
      </div>

      {traces.length === 0 ? (
        <div className="py-8 text-center text-sm">
          <p className="text-gray-500">No traces yet</p>
          <p className="mt-1 text-xs text-gray-400 dark:text-gray-600">
            Trigger a turn, trade, or group invite to see timings
          </p>
        </div>
      ) : (
        <>
          <div className="overflow-x-auto rounded-lg border border-gray-200 dark:border-gray-800">
            <table className="w-full text-xs">
              <thead>
                <tr className="border-b border-gray-200 text-gray-500 dark:border-gray-800">
                  <th className="px-2 py-2 text-left font-medium">Time</th>
                  <th className="px-2 py-2 text-left font-medium">Event</th>
                  <th className="px-2 py-2 text-left font-medium">Character</th>
                  <th className="px-2 py-2 text-right font-medium">Duration</th>
                </tr>
              </thead>
              <tbody>
                {reversed.map((t, i) => {
                  const total = t.t_focus_done_ms - t.t_notification_ms;
                  const isSystemEvent =
                    t.event_type === "notification_center_restart" ||
                    t.event_type === "listener_reconnect";
                  return (
                    <tr
                      key={i}
                      className="border-b border-gray-200/50 hover:bg-gray-50 dark:border-gray-800/50 dark:hover:bg-gray-900/40"
                    >
                      <td className="px-2 py-1.5 font-mono text-gray-500">
                        {formatTime(t.t_notification_ms)}
                      </td>
                      <td className="px-2 py-1.5">
                        <span
                          className={`rounded px-1.5 py-0.5 text-[10px] font-medium ${eventBadge(t.event_type)}`}
                        >
                          {eventLabel(t.event_type)}
                        </span>
                      </td>
                      <td className="max-w-[80px] truncate px-2 py-1.5 text-gray-700 dark:text-gray-300">
                        {t.character_name}
                      </td>
                      <td
                        className={`px-2 py-1.5 text-right font-mono font-semibold ${isSystemEvent ? "text-gray-400 dark:text-gray-600" : latencyColor(total)}`}
                      >
                        {isSystemEvent ? "-" : `${total}ms`}
                      </td>
                    </tr>
                  );
                })}
              </tbody>
            </table>
          </div>
        </>
      )}
    </div>
  );
}
