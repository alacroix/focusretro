import { useTranslation } from "react-i18next";
import { openSettings, requestScreenRecording } from "../lib/commands";

interface Props {
  accessibility: boolean;
  screenRecording: boolean;
  onRecheck: () => void;
}

function StatusBadge({ granted }: { granted: boolean }) {
  const { t } = useTranslation();
  return (
    <span
      className={`text-[11px] px-2 py-0.5 rounded font-medium ${
        granted
          ? "bg-emerald-50 text-emerald-600 dark:bg-emerald-600/20 dark:text-emerald-400"
          : "bg-red-50 text-red-600 dark:bg-red-900/40 dark:text-red-400"
      }`}
    >
      {granted ? t("setup.status_granted") : t("setup.status_missing")}
    </span>
  );
}

function PermissionCard({
  granted,
  icon,
  name,
  why,
  note,
  actions,
}: {
  granted: boolean;
  icon: React.ReactNode;
  name: string;
  why: string;
  note?: string;
  actions: React.ReactNode;
}) {
  return (
    <div
      className={`mx-4 mt-3 p-4 rounded-lg border ${
        granted
          ? "border-emerald-200 bg-emerald-50 dark:border-emerald-800/50 dark:bg-emerald-950/20"
          : "border-amber-200 bg-amber-50 dark:border-amber-800/50 dark:bg-amber-950/20"
      }`}
    >
      <div className="flex items-start gap-3">
        <div
          className={`mt-0.5 shrink-0 ${
            granted ? "text-emerald-600 dark:text-emerald-400" : "text-amber-600 dark:text-amber-400"
          }`}
        >
          {icon}
        </div>
        <div className="flex-1 min-w-0">
          <div className="flex items-center gap-2 mb-1">
            <span className="text-sm font-medium text-gray-800 dark:text-gray-200">{name}</span>
            <StatusBadge granted={granted} />
          </div>
          <p className="text-xs text-gray-500 leading-relaxed">{why}</p>
          {note && (
            <p className="text-[11px] text-gray-400 dark:text-gray-600 mt-1.5 leading-relaxed">
              {note}
            </p>
          )}
        </div>
      </div>
      {!granted && (
        <div className="flex gap-2 mt-3 flex-wrap">{actions}</div>
      )}
    </div>
  );
}

function PermissionsSetup({ accessibility, screenRecording, onRecheck }: Props) {
  const { t } = useTranslation();
  const allGranted = accessibility && screenRecording;

  return (
    <div className="min-h-screen bg-white dark:bg-gray-950 text-gray-900 dark:text-gray-100 flex flex-col">
      <div className="px-4 pt-4">
        <h2 className="text-sm font-semibold text-gray-800 dark:text-gray-200">
          {t("setup.title")}
        </h2>
        <p className="text-xs text-gray-500 mt-0.5">{t("setup.subtitle")}</p>
      </div>

      <PermissionCard
        granted={accessibility}
        icon={
          <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round">
            <circle cx="12" cy="12" r="10" />
            <path d="M12 8v4M12 16h.01" />
          </svg>
        }
        name={t("setup.accessibility_name")}
        why={t("setup.accessibility_why")}
        note={t("setup.accessibility_note")}
        actions={
          <button
            type="button"
            onClick={() => openSettings("accessibility")}
            className="text-xs px-3 py-1.5 rounded-md bg-amber-100 hover:bg-amber-200 text-amber-800 dark:bg-amber-800/40 dark:hover:bg-amber-700/50 dark:text-amber-200 transition-colors"
          >
            {t("setup.open_settings")}
          </button>
        }
      />

      <PermissionCard
        granted={screenRecording}
        icon={
          <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round">
            <rect x="2" y="4" width="20" height="14" rx="2" />
            <path d="M8 20h8M12 18v2" />
          </svg>
        }
        name={t("setup.screen_recording_name")}
        why={t("setup.screen_recording_why")}
        actions={
          <>
            <button
              type="button"
              onClick={() => requestScreenRecording()}
              className="text-xs px-3 py-1.5 rounded-md bg-indigo-100 hover:bg-indigo-200 text-indigo-800 dark:bg-indigo-700/50 dark:hover:bg-indigo-600/60 dark:text-indigo-200 transition-colors"
            >
              {t("setup.request_permission")}
            </button>
            <button
              type="button"
              onClick={() => openSettings("screen_recording")}
              className="text-xs px-3 py-1.5 rounded-md bg-amber-100 hover:bg-amber-200 text-amber-800 dark:bg-amber-800/40 dark:hover:bg-amber-700/50 dark:text-amber-200 transition-colors"
            >
              {t("setup.open_settings")}
            </button>
          </>
        }
      />

      <div className="flex items-center justify-between mx-4 mt-6 pt-4 border-t border-gray-200 dark:border-gray-800">
        <button
          type="button"
          onClick={onRecheck}
          className="text-xs px-3 py-1.5 rounded-md bg-gray-100 hover:bg-gray-200 dark:bg-gray-800 dark:hover:bg-gray-700 text-gray-700 dark:text-gray-300 transition-colors"
        >
          {t("setup.check_again")}
        </button>
        <button
          type="button"
          onClick={onRecheck}
          disabled={!allGranted}
          className={`text-xs px-4 py-1.5 rounded-md font-medium transition-colors ${
            allGranted
              ? "bg-indigo-600 hover:bg-indigo-500 text-white"
              : "bg-gray-100 dark:bg-gray-800 text-gray-400 dark:text-gray-600 cursor-not-allowed"
          }`}
        >
          {t("setup.continue")}
        </button>
      </div>
    </div>
  );
}

export default PermissionsSetup;
