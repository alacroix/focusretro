import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { listen } from "@tauri-apps/api/event";
import { check, type Update } from "@tauri-apps/plugin-updater";
import { relaunch } from "@tauri-apps/plugin-process";
import AccountList from "./components/AccountList";
import MessageList from "./components/MessageList";
import Settings from "./components/Settings";
import DebugPanel from "./components/DebugPanel";
import PermissionsSetup from "./components/PermissionsSetup";
import {
  AccountView,
  HotkeyBinding,
  checkPermissions,
  getLanguage,
  getHotkeys,
  refreshAccounts,
  getShowDebug,
} from "./lib/commands";
import i18n from "./i18n";

type Tab = "accounts" | "messages" | "settings" | "debug";

function App() {
  const { t } = useTranslation();
  const [accounts, setAccounts] = useState<AccountView[]>([]);
  const [permissionsChecked, setPermissionsChecked] = useState(false);
  const [hasAccessibility, setHasAccessibility] = useState(false);
  const [hasScreenRecording, setHasScreenRecording] = useState(false);
  const [tab, setTab] = useState<Tab>("accounts");
  const [showDebug, setShowDebug] = useState(false);
  const [hotkeys, setHotkeys] = useState<HotkeyBinding[]>([]);
  const [pendingUpdate, setPendingUpdate] = useState<Update | null>(null);
  const [updateStatus, setUpdateStatus] = useState<"idle" | "downloading" | "done">("idle");

  useEffect(() => {
    refreshAccounts().then(setAccounts);
    checkPermissions().then((p) => {
      setHasAccessibility(p.accessibility);
      setHasScreenRecording(p.screen_recording);
      setPermissionsChecked(true);
    });
    getLanguage().then((lang) => {
      if (lang && lang !== i18n.language) {
        i18n.changeLanguage(lang);
      }
    });
    getHotkeys().then(setHotkeys);
    getShowDebug().then(setShowDebug);
    check().then((u) => { if (u?.available) setPendingUpdate(u); }).catch(() => {});

    const unlistenAccounts = listen<AccountView[]>("accounts-updated", (e) => {
      setAccounts(e.payload);
    });

const unlistenHotkeys = listen<HotkeyBinding[]>("hotkeys-updated", (e) => {
      setHotkeys(e.payload);
    });

    return () => {
      unlistenAccounts.then((f) => f());
unlistenHotkeys.then((f) => f());
    };
  }, []);

  const visibleTabs = (["accounts", "messages", "settings", ...(showDebug ? ["debug"] : [])] as Tab[]);

  const tabLabels: Record<Tab, string> = {
    accounts: t("tabs.accounts"),
    messages: t("tabs.messages"),
    settings: t("tabs.settings"),
    debug: t("tabs.debug"),
  };

  const formatHotkeyLabel = (hk: HotkeyBinding): string => {
    const parts: string[] = [];
    if (hk.cmd) parts.push("Cmd");
    if (hk.ctrl) parts.push("Ctrl");
    if (hk.alt) parts.push("Alt");
    if (hk.shift) parts.push("Shift");
    parts.push(hk.key.replace("Key", "").replace("Digit", ""));
    return parts.join("+");
  };

  const hotkeyLabelFor = (action: string) => {
    const hk = hotkeys.find((h) => h.action === action);
    return hk ? formatHotkeyLabel(hk) : "";
  };

  const handleInstall = async () => {
    if (!pendingUpdate) return;
    setUpdateStatus("downloading");
    await pendingUpdate.downloadAndInstall();
    setUpdateStatus("done");
  };

  const handleRecheck = () => {
    checkPermissions().then((p) => {
      setHasAccessibility(p.accessibility);
      setHasScreenRecording(p.screen_recording);
    });
  };

  if (!permissionsChecked) {
    return <div className="min-h-screen bg-gray-950" />;
  }

  if (!hasAccessibility || !hasScreenRecording) {
    return (
      <PermissionsSetup
        accessibility={hasAccessibility}
        screenRecording={hasScreenRecording}
        onRecheck={handleRecheck}
      />
    );
  }

  return (
    <div className="min-h-screen bg-gray-950 text-gray-100 flex flex-col">
{pendingUpdate && updateStatus === "idle" && (
        <div className="mx-4 mt-3 px-3 py-2 bg-indigo-950/50 border border-indigo-800/50 rounded-lg text-sm text-indigo-200 flex items-center justify-between gap-2">
          <span>{t("update.available", { version: pendingUpdate.version })}</span>
          <button
            onClick={handleInstall}
            className="px-2 py-0.5 bg-indigo-700 hover:bg-indigo-600 text-white rounded text-xs font-medium shrink-0"
          >
            {t("update.install")}
          </button>
        </div>
      )}

      {updateStatus === "downloading" && (
        <div className="mx-4 mt-3 px-3 py-2 bg-indigo-950/50 border border-indigo-800/50 rounded-lg text-sm text-indigo-300 animate-pulse">
          {t("update.downloading")}
        </div>
      )}

      {updateStatus === "done" && (
        <div className="mx-4 mt-3 px-3 py-2 bg-emerald-950/50 border border-emerald-800/50 rounded-lg text-sm text-emerald-200 flex items-center justify-between gap-2">
          <span>{t("update.ready")}</span>
          <button
            onClick={relaunch}
            className="px-2 py-0.5 bg-emerald-700 hover:bg-emerald-600 text-white rounded text-xs font-medium shrink-0"
          >
            {t("update.restart")}
          </button>
        </div>
      )}

      {hotkeys.length > 0 && (
        <div className="mx-4 mt-3 px-4 py-3 bg-gray-900/70 border border-gray-800 rounded-lg text-xs text-gray-300">
          <p className="text-gray-500 mb-3">{t("hotkeys.label")}</p>
          <div className="grid grid-cols-3 gap-x-4 gap-y-2">
            <div className="flex flex-col items-center gap-1">
              <kbd className="px-2 py-1 bg-red-950/40 border border-red-500/50 rounded text-red-300 w-fit text-sm font-mono">
                {hotkeyLabelFor("prev")}
              </kbd>
              <span>{t("accounts.previous")}</span>
            </div>
            <div className="flex flex-col items-center gap-1">
              <kbd className="px-2 py-1 bg-emerald-950/40 border border-emerald-500/50 rounded text-emerald-300 w-fit text-sm font-mono">
                {hotkeyLabelFor("next")}
              </kbd>
              <span>{t("accounts.next")}</span>
            </div>
            <div className="flex flex-col items-center gap-1">
              <kbd className="px-2 py-1 bg-amber-950/40 border border-amber-500/50 rounded text-amber-300 w-fit text-sm font-mono">
                {hotkeyLabelFor("principal")}
              </kbd>
              <span>{t("accounts.principal")}</span>
            </div>
          </div>
        </div>
      )}

      <div className="flex border-b border-gray-800 mx-4 mt-3">
        {visibleTabs.map((t) => (
          <button
            key={t}
            onClick={() => setTab(t)}
            className={`px-3 py-2 text-xs font-medium transition-colors ${
              tab === t
                ? "text-gray-100 border-b-2 border-indigo-500"
                : "text-gray-500 hover:text-gray-300"
            }`}
          >
            {tabLabels[t]}
          </button>
        ))}
      </div>

      <main className="flex-1 px-4 py-3 overflow-y-auto">
        {tab === "accounts" && (
          <AccountList
            accounts={accounts}
            onRefresh={() => refreshAccounts().then(setAccounts)}
            onUpdate={setAccounts}
          />
        )}
        {tab === "messages" && <MessageList />}
        {tab === "settings" && <Settings showDebug={showDebug} onToggleDebug={setShowDebug} />}
        {tab === "debug" && <DebugPanel />}
      </main>
    </div>
  );
}

export default App;
