import { useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { TraceEntry, getTraces, clearTraces, getNotifMode } from "../lib/commands";

function latencyColor(ms: number): string {
  if (ms < 50) return "text-emerald-400";
  if (ms < 150) return "text-amber-400";
  return "text-red-400";
}

function formatTime(ms: number): string {
  const d = new Date(ms);
  return d.toLocaleTimeString("fr-FR", { hour: "2-digit", minute: "2-digit", second: "2-digit" });
}

function eventLabel(type: string): string {
  if (type === "turn") return "Turn";
  if (type === "group_invite") return "Group invite";
  if (type === "trade") return "Trade";
  return type;
}

function eventBadge(type: string): string {
  if (type === "turn") return "bg-indigo-900/60 text-indigo-300 border border-indigo-700/50";
  if (type === "group_invite") return "bg-emerald-900/60 text-emerald-300 border border-emerald-700/50";
  if (type === "trade") return "bg-amber-900/60 text-amber-300 border border-amber-700/50";
  return "bg-gray-800 text-gray-300";
}


export default function DebugPanel() {
  const [traces, setTraces] = useState<TraceEntry[]>([]);
  const [notifMode, setNotifMode] = useState<string>("unknown");
  const reload = () => getTraces().then(setTraces);

  useEffect(() => {
    reload();
    getNotifMode().then(setNotifMode);
    const unlistenTrace = listen("trace-added", reload);
    const unlistenMode = listen<string>("notif-mode-changed", (e) => setNotifMode(e.payload));
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
          <h2 className="text-xs font-semibold text-gray-400 uppercase tracking-wider">Traces</h2>
          <span className={`px-1.5 py-0.5 rounded text-[10px] font-medium border ${
            notifMode === "event"
              ? "bg-emerald-900/60 text-emerald-300 border-emerald-700/50"
              : notifMode === "poll"
              ? "bg-amber-900/60 text-amber-300 border-amber-700/50"
              : "bg-gray-800 text-gray-500 border-gray-700"
          }`}>
            {notifMode === "event" ? "event-driven" : notifMode === "poll" ? "polling 20ms" : notifMode}
          </span>
        </div>
        <button
          onClick={handleClear}
          className="px-2 py-1 text-xs bg-gray-800 hover:bg-gray-700 text-gray-300 rounded border border-gray-700"
        >
          Clear
        </button>
      </div>

      {traces.length === 0 ? (
        <div className="text-center py-8 text-gray-600 text-sm">
          <p className="text-gray-500">No traces yet</p>
          <p className="text-gray-600 text-xs mt-1">Trigger a turn, trade, or group invite to see timings</p>
        </div>
      ) : (
        <>
          <div className="overflow-x-auto rounded-lg border border-gray-800">
            <table className="w-full text-xs">
              <thead>
                <tr className="border-b border-gray-800 text-gray-500">
                  <th className="text-left px-2 py-2 font-medium">Time</th>
                  <th className="text-left px-2 py-2 font-medium">Event</th>
                  <th className="text-left px-2 py-2 font-medium">Character</th>
                  <th className="text-right px-2 py-2 font-medium">Duration</th>
                </tr>
              </thead>
              <tbody>
                {reversed.map((t, i) => {
                  const total = t.t_focus_done_ms - t.t_notification_ms;
                  return (
                    <tr key={i} className="border-b border-gray-800/50 hover:bg-gray-900/40">
                      <td className="px-2 py-1.5 text-gray-500 font-mono">{formatTime(t.t_notification_ms)}</td>
                      <td className="px-2 py-1.5">
                        <span className={`px-1.5 py-0.5 rounded text-[10px] font-medium ${eventBadge(t.event_type)}`}>
                          {eventLabel(t.event_type)}
                        </span>
                      </td>
                      <td className="px-2 py-1.5 text-gray-300 truncate max-w-[80px]">{t.character_name}</td>
                      <td className={`px-2 py-1.5 text-right font-mono font-semibold ${latencyColor(total)}`}>{total}ms</td>
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
