import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { listen } from "@tauri-apps/api/event";
import type { Update } from "@tauri-apps/plugin-updater";
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
  getUpdateConsent,
  setUpdateConsent,
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
  const [hasInputMonitoring, setHasInputMonitoring] = useState(false);
  const [tab, setTab] = useState<Tab>("accounts");
  const [showDebug, setShowDebug] = useState(false);
  const [hotkeys, setHotkeys] = useState<HotkeyBinding[]>([]);
  const [focusedName, setFocusedName] = useState<string | null>(null);
  const [pendingUpdate, setPendingUpdate] = useState<Update | null>(null);
  const [updateStatus, setUpdateStatus] = useState<"idle" | "downloading" | "done">("idle");
  const [theme, setThemeState] = useState("system");
  const [updateConsent, setUpdateConsentState] = useState<boolean | null | undefined>(undefined);
  const [showConsentModal, setShowConsentModal] = useState(false);

  useEffect(() => {
    refreshAccounts().then(setAccounts);
    checkPermissions().then((p) => {
      setHasAccessibility(p.accessibility);
      setHasScreenRecording(p.screen_recording);
      setHasInputMonitoring(p.input_monitoring);
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
    if (import.meta.env.VITE_UPDATER !== "false") {
      getUpdateConsent().then(async (consent) => {
        setUpdateConsentState(consent ?? null);
        if (consent === null || consent === undefined) {
          setShowConsentModal(true);
        } else if (consent === true) {
          const { check } = await import("@tauri-apps/plugin-updater");
          check().then((u) => { if (u?.available) setPendingUpdate(u); }).catch(() => {});
        }
      });
    }

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

  const handleUpdateConsentChange = async (consent: boolean) => {
    setUpdateConsentState(consent);
    setUpdateConsent(consent).catch(() => {});
    if (import.meta.env.VITE_UPDATER !== "false" && consent) {
      const { check } = await import("@tauri-apps/plugin-updater");
      check().then((u) => { if (u?.available) setPendingUpdate(u); }).catch(() => {});
    }
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
    const ARROW_LABELS: Record<string, string> = {
      ArrowLeft: "←", ArrowRight: "→", ArrowUp: "↑", ArrowDown: "↓",
    };
    const MOUSE_LABELS: Record<string, string> = { Mouse4: "Mouse 4", Mouse5: "Mouse 5" };
    parts.push(MOUSE_LABELS[hk.key] ?? ARROW_LABELS[hk.key] ?? hk.key.replace("Key", "").replace("Digit", ""));
    return parts.join(" + ");
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
      setHasInputMonitoring(p.input_monitoring);
    });
  };

  if (!permissionsChecked) {
    return <div className="min-h-screen bg-white dark:bg-gray-950" />;
  }

  if (!hasAccessibility || !hasScreenRecording || !hasInputMonitoring) {
    return (
      <PermissionsSetup
        accessibility={hasAccessibility}
        screenRecording={hasScreenRecording}
        inputMonitoring={hasInputMonitoring}
        onRecheck={handleRecheck}
      />
    );
  }

  if (import.meta.env.VITE_UPDATER !== "false" && showConsentModal) {
    return (
      <div className="min-h-screen bg-white dark:bg-gray-950 flex items-center justify-center p-6">
        <div className="max-w-sm w-full bg-white dark:bg-gray-900 border border-gray-200 dark:border-gray-800 rounded-xl p-5 shadow-sm">
          <h2 className="text-sm font-semibold text-gray-900 dark:text-gray-100 mb-2">
            {t("update.consent_title")}
          </h2>
          <p className="text-xs text-gray-500 dark:text-gray-400 mb-5">
            {t("update.consent_body")}
          </p>
          <div className="flex gap-2">
            <button
              onClick={() => {
                handleUpdateConsentChange(true);
                setShowConsentModal(false);
              }}
              className="flex-1 px-3 py-2 bg-brand-600 hover:bg-brand-500 text-white rounded-lg text-xs font-medium transition-colors"
            >
              {t("update.consent_yes")}
            </button>
            <button
              onClick={() => {
                handleUpdateConsentChange(false);
                setShowConsentModal(false);
              }}
              className="flex-1 px-3 py-2 bg-gray-100 hover:bg-gray-200 dark:bg-gray-800 dark:hover:bg-gray-700 text-gray-700 dark:text-gray-300 rounded-lg text-xs font-medium transition-colors"
            >
              {t("update.consent_no")}
            </button>
          </div>
        </div>
      </div>
    );
  }

  return (
    <div className="min-h-screen bg-white dark:bg-gray-950 text-gray-900 dark:text-gray-100 flex flex-col">
{pendingUpdate && updateStatus === "idle" && (
        <div className="mx-4 mt-3 px-3 py-2 bg-brand-50 border border-brand-200 dark:bg-brand-900/20 dark:border-brand-800/50 rounded-lg text-sm text-brand-700 dark:text-brand-200 flex items-center justify-between gap-2">
          <span>{t("update.available", { version: pendingUpdate.version })}</span>
          <button
            onClick={handleInstall}
            className="px-2 py-0.5 bg-brand-600 hover:bg-brand-500 text-white rounded text-xs font-medium shrink-0"
          >
            {t("update.install")}
          </button>
        </div>
      )}

      {updateStatus === "downloading" && (
        <div className="mx-4 mt-3 px-3 py-2 bg-brand-50 border border-brand-200 dark:bg-brand-900/20 dark:border-brand-800/50 rounded-lg text-sm text-brand-600 dark:text-brand-300 animate-pulse">
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
                ? "text-gray-900 dark:text-gray-100 border-b-2 border-brand-500"
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
              <div className="mt-auto pt-4 flex items-center justify-between text-[10px] text-gray-500 dark:text-gray-400">
                <span className="flex flex-col items-center gap-1.5">
                  <kbd className="px-1.5 py-0.5 bg-gray-100 dark:bg-gray-800 border border-gray-300 dark:border-gray-600 rounded font-mono text-gray-700 dark:text-gray-200 text-[10px] whitespace-nowrap">
                    {hotkeyLabelFor("prev")}
                  </kbd>
                  {t("accounts.previous")}
                </span>
                <span className="flex flex-col items-center gap-1.5">
                  <kbd className="px-1.5 py-0.5 bg-gray-100 dark:bg-gray-800 border border-gray-300 dark:border-gray-600 rounded font-mono text-gray-700 dark:text-gray-200 text-[10px] whitespace-nowrap">
                    {hotkeyLabelFor("next")}
                  </kbd>
                  {t("accounts.next")}
                </span>
                <span className="flex flex-col items-center gap-1.5">
                  <kbd className="px-1.5 py-0.5 bg-gray-100 dark:bg-gray-800 border border-gray-300 dark:border-gray-600 rounded font-mono text-gray-700 dark:text-gray-200 text-[10px] whitespace-nowrap">
                    {hotkeyLabelFor("principal")}
                  </kbd>
                  {t("accounts.principal")}
                </span>
                {hotkeyLabelFor("radial") && (
                  <span className="flex flex-col items-center gap-1.5">
                    <kbd className="px-1.5 py-0.5 bg-gray-100 dark:bg-gray-800 border border-gray-300 dark:border-gray-600 rounded font-mono text-gray-700 dark:text-gray-200 text-[10px] whitespace-nowrap">
                      {hotkeyLabelFor("radial")}
                    </kbd>
                    {t("hotkeys.radial")}
                  </span>
                )}
              </div>
            )}
          </div>
        )}
        {tab === "messages" && <MessageList />}
        {tab === "settings" && <Settings showDebug={showDebug} onToggleDebug={setShowDebug} theme={theme} onThemeChange={handleThemeChange} updateConsent={updateConsent} onUpdateConsentChange={handleUpdateConsentChange} />}
        {tab === "debug" && <DebugPanel />}
      </main>
    </div>
  );
}

export default App;
