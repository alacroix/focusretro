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
  getTheme,
  setTheme,
} from "./lib/commands";
import i18n from "./i18n";

type Tab = "accounts" | "messages" | "settings" | "debug";

function applyThemeClass(theme: string) {
  const html = document.documentElement;
  if (theme === "dark") {
    html.classList.add("dark");
  } else if (theme === "light") {
    html.classList.remove("dark");
  } else {
    const prefersDark = window.matchMedia("(prefers-color-scheme: dark)").matches;
    html.classList.toggle("dark", prefersDark);
  }
}

function App() {
  const { t } = useTranslation();
  const [accounts, setAccounts] = useState<AccountView[]>([]);
  const [permissionsChecked, setPermissionsChecked] = useState(false);
  const [hasAccessibility, setHasAccessibility] = useState(false);
  const [hasScreenRecording, setHasScreenRecording] = useState(false);
  const [tab, setTab] = useState<Tab>("accounts");
  const [showDebug, setShowDebug] = useState(false);
  const [hotkeys, setHotkeys] = useState<HotkeyBinding[]>([]);
  const [focusedName, setFocusedName] = useState<string | null>(null);
  const [pendingUpdate, setPendingUpdate] = useState<Update | null>(null);
  const [updateStatus, setUpdateStatus] = useState<"idle" | "downloading" | "done">("idle");
  const [theme, setThemeState] = useState("system");

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
    getTheme().then((t) => {
      setThemeState(t);
      applyThemeClass(t);
    });
    check().then((u) => { if (u?.available) setPendingUpdate(u); }).catch(() => {});

    const unlistenAccounts = listen<AccountView[]>("accounts-updated", (e) => {
      setAccounts(e.payload);
    });

    const unlistenHotkeys = listen<HotkeyBinding[]>("hotkeys-updated", (e) => {
      setHotkeys(e.payload);
    });

    const unlistenFocus = listen<string>("focus-changed", (e) => {
      setFocusedName(e.payload);
    });

    return () => {
      unlistenAccounts.then((f) => f());
      unlistenHotkeys.then((f) => f());
      unlistenFocus.then((f) => f());
    };
  }, []);

  useEffect(() => {
    applyThemeClass(theme);

    if (theme === "system") {
      const mq = window.matchMedia("(prefers-color-scheme: dark)");
      const handler = () => applyThemeClass("system");
      mq.addEventListener("change", handler);
      return () => mq.removeEventListener("change", handler);
    }
  }, [theme]);

  const handleThemeChange = (newTheme: string) => {
    applyThemeClass(newTheme);
    setThemeState(newTheme);
    setTheme(newTheme).catch(() => {});
  };

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
    return <div className="min-h-screen bg-white dark:bg-gray-950" />;
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
    <div className="min-h-screen bg-white dark:bg-gray-950 text-gray-900 dark:text-gray-100 flex flex-col">
{pendingUpdate && updateStatus === "idle" && (
        <div className="mx-4 mt-3 px-3 py-2 bg-indigo-50 border border-indigo-200 dark:bg-indigo-950/50 dark:border-indigo-800/50 rounded-lg text-sm text-indigo-700 dark:text-indigo-200 flex items-center justify-between gap-2">
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
        <div className="mx-4 mt-3 px-3 py-2 bg-indigo-50 border border-indigo-200 dark:bg-indigo-950/50 dark:border-indigo-800/50 rounded-lg text-sm text-indigo-600 dark:text-indigo-300 animate-pulse">
          {t("update.downloading")}
        </div>
      )}

      {updateStatus === "done" && (
        <div className="mx-4 mt-3 px-3 py-2 bg-emerald-50 border border-emerald-200 dark:bg-emerald-950/50 dark:border-emerald-800/50 rounded-lg text-sm text-emerald-700 dark:text-emerald-200 flex items-center justify-between gap-2">
          <span>{t("update.ready")}</span>
          <button
            onClick={relaunch}
            className="px-2 py-0.5 bg-emerald-700 hover:bg-emerald-600 text-white rounded text-xs font-medium shrink-0"
          >
            {t("update.restart")}
          </button>
        </div>
      )}

      <div className="flex border-b border-gray-200 dark:border-gray-800 mx-4 mt-3">
        {visibleTabs.map((t) => (
          <button
            key={t}
            onClick={() => setTab(t)}
            className={`px-3 py-2 text-xs font-medium transition-colors ${
              tab === t
                ? "text-gray-900 dark:text-gray-100 border-b-2 border-indigo-500"
                : "text-gray-500 hover:text-gray-700 dark:hover:text-gray-300"
            }`}
          >
            {tabLabels[t]}
          </button>
        ))}
      </div>

      <main className="flex-1 px-4 py-3 overflow-y-auto flex flex-col">
        {tab === "accounts" && (
          <div className="flex flex-col flex-1">
            <AccountList
              accounts={accounts}
              focusedName={focusedName}
              onRefresh={() => refreshAccounts().then(setAccounts)}
              onUpdate={setAccounts}
              onFocused={setFocusedName}
            />
            {hotkeys.length > 0 && (
              <div className="mt-auto pt-4 flex items-center justify-center gap-5 text-[10px] text-gray-500 dark:text-gray-400">
                <span className="flex items-center gap-1.5">
                  <kbd className="px-1.5 py-0.5 bg-gray-100 dark:bg-gray-800 border border-gray-300 dark:border-gray-600 rounded font-mono text-gray-700 dark:text-gray-200 text-[10px]">
                    {hotkeyLabelFor("prev")}
                  </kbd>
                  {t("accounts.previous")}
                </span>
                <span className="flex items-center gap-1.5">
                  <kbd className="px-1.5 py-0.5 bg-gray-100 dark:bg-gray-800 border border-gray-300 dark:border-gray-600 rounded font-mono text-gray-700 dark:text-gray-200 text-[10px]">
                    {hotkeyLabelFor("next")}
                  </kbd>
                  {t("accounts.next")}
                </span>
                <span className="flex items-center gap-1.5">
                  <kbd className="px-1.5 py-0.5 bg-gray-100 dark:bg-gray-800 border border-gray-300 dark:border-gray-600 rounded font-mono text-gray-700 dark:text-gray-200 text-[10px]">
                    {hotkeyLabelFor("principal")}
                  </kbd>
                  {t("accounts.principal")}
                </span>
              </div>
            )}
          </div>
        )}
        {tab === "messages" && <MessageList />}
        {tab === "settings" && <Settings showDebug={showDebug} onToggleDebug={setShowDebug} theme={theme} onThemeChange={handleThemeChange} />}
        {tab === "debug" && <DebugPanel />}
      </main>
    </div>
  );
}

export default App;
