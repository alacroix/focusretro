import { useState } from "react";
import { useTranslation } from "react-i18next";

import { openSettings, requestScreenRecording, requestInputMonitoring } from "../lib/commands";

interface Props {
  accessibility: boolean;
  screenRecording: boolean;
  inputMonitoring: boolean;
  onRecheck: () => void;
}

function StatusBadge({ granted }: { granted: boolean }) {
  const { t } = useTranslation();
  return (
    <span
      className={`rounded px-2 py-0.5 text-[11px] font-medium ${
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
      className={`mx-4 mt-3 rounded-lg border p-4 ${
        granted
          ? "border-emerald-200 bg-emerald-50 dark:border-emerald-800/50 dark:bg-emerald-950/20"
          : "border-amber-200 bg-amber-50 dark:border-amber-800/50 dark:bg-amber-950/20"
      }`}
    >
      <div className="flex items-start gap-3">
        <div
          className={`mt-0.5 shrink-0 ${
            granted
              ? "text-emerald-600 dark:text-emerald-400"
              : "text-amber-600 dark:text-amber-400"
          }`}
        >
          {icon}
        </div>
        <div className="min-w-0 flex-1">
          <div className="mb-1 flex items-center gap-2">
            <span className="text-sm font-medium text-gray-800 dark:text-gray-200">{name}</span>
            <StatusBadge granted={granted} />
          </div>
          <p className="text-xs leading-relaxed text-gray-500">{why}</p>
          {note && (
            <p className="mt-1.5 text-[11px] leading-relaxed text-gray-400 dark:text-gray-600">
              {note}
            </p>
          )}
        </div>
      </div>
      {!granted && <div className="mt-3 flex flex-wrap gap-2">{actions}</div>}
    </div>
  );
}

function PermissionsSetup({ accessibility, screenRecording, inputMonitoring, onRecheck }: Props) {
  const { t } = useTranslation();
  const [error, setError] = useState<string | null>(null);
  const allGranted = accessibility && screenRecording && inputMonitoring;

  const handleRecheck = () => {
    setError(null);
    onRecheck();
  };

  return (
    <div className="flex min-h-screen flex-col bg-white text-gray-900 dark:bg-gray-950 dark:text-gray-100">
      <div className="px-4 pt-4">
        <h2 className="text-sm font-semibold text-gray-800 dark:text-gray-200">
          {t("setup.title")}
        </h2>
        <p className="mt-0.5 text-xs text-gray-500">{t("setup.subtitle")}</p>
      </div>

      <PermissionCard
        granted={accessibility}
        icon={
          <svg
            width="20"
            height="20"
            viewBox="0 0 24 24"
            fill="none"
            stroke="currentColor"
            strokeWidth="1.5"
            strokeLinecap="round"
            strokeLinejoin="round"
          >
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
            onClick={() =>
              openSettings("accessibility").catch(() => setError(t("setup.request_failed")))
            }
            className="cursor-pointer rounded-md bg-amber-100 px-3 py-1.5 text-xs text-amber-800 transition-colors hover:bg-amber-200 dark:bg-amber-800/40 dark:text-amber-200 dark:hover:bg-amber-700/50"
          >
            {t("setup.open_settings")}
          </button>
        }
      />

      <PermissionCard
        granted={screenRecording}
        icon={
          <svg
            width="20"
            height="20"
            viewBox="0 0 24 24"
            fill="none"
            stroke="currentColor"
            strokeWidth="1.5"
            strokeLinecap="round"
            strokeLinejoin="round"
          >
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
              onClick={() =>
                requestScreenRecording().catch(() => setError(t("setup.request_failed")))
              }
              className="cursor-pointer rounded-md bg-brand-100 px-3 py-1.5 text-xs text-brand-800 transition-colors hover:bg-brand-200 dark:bg-brand-700/50 dark:text-brand-200 dark:hover:bg-brand-600/60"
            >
              {t("setup.request_permission")}
            </button>
            <button
              type="button"
              onClick={() =>
                openSettings("screen_recording").catch(() => setError(t("setup.request_failed")))
              }
              className="cursor-pointer rounded-md bg-amber-100 px-3 py-1.5 text-xs text-amber-800 transition-colors hover:bg-amber-200 dark:bg-amber-800/40 dark:text-amber-200 dark:hover:bg-amber-700/50"
            >
              {t("setup.open_settings")}
            </button>
          </>
        }
      />

      <PermissionCard
        granted={inputMonitoring}
        icon={
          <svg
            width="20"
            height="20"
            viewBox="0 0 24 24"
            fill="none"
            stroke="currentColor"
            strokeWidth="1.5"
            strokeLinecap="round"
            strokeLinejoin="round"
          >
            <path d="M12 1a3 3 0 0 0-3 3v8a3 3 0 0 0 6 0V4a3 3 0 0 0-3-3z" />
            <path d="M19 10v2a7 7 0 0 1-14 0v-2M12 19v4M8 23h8" />
          </svg>
        }
        name={t("setup.input_monitoring_name")}
        why={t("setup.input_monitoring_why")}
        actions={
          <>
            <button
              type="button"
              onClick={() =>
                requestInputMonitoring().catch(() => setError(t("setup.request_failed")))
              }
              className="cursor-pointer rounded-md bg-brand-100 px-3 py-1.5 text-xs text-brand-800 transition-colors hover:bg-brand-200 dark:bg-brand-700/50 dark:text-brand-200 dark:hover:bg-brand-600/60"
            >
              {t("setup.request_permission")}
            </button>
            <button
              type="button"
              onClick={() =>
                openSettings("input_monitoring").catch(() => setError(t("setup.request_failed")))
              }
              className="cursor-pointer rounded-md bg-amber-100 px-3 py-1.5 text-xs text-amber-800 transition-colors hover:bg-amber-200 dark:bg-amber-800/40 dark:text-amber-200 dark:hover:bg-amber-700/50"
            >
              {t("setup.open_settings")}
            </button>
          </>
        }
      />

      {error && <p className="mx-4 mt-2 text-xs text-red-600 dark:text-red-400">{error}</p>}

      <div className="mx-4 mt-6 flex items-center justify-between border-t border-gray-200 pt-4 dark:border-gray-800">
        <button
          type="button"
          onClick={handleRecheck}
          className="cursor-pointer rounded-md bg-gray-100 px-3 py-1.5 text-xs text-gray-700 transition-colors hover:bg-gray-200 dark:bg-gray-800 dark:text-gray-300 dark:hover:bg-gray-700"
        >
          {t("setup.check_again")}
        </button>
        <button
          type="button"
          onClick={handleRecheck}
          disabled={!allGranted}
          className={`rounded-md px-4 py-1.5 text-xs font-medium transition-colors ${
            allGranted
              ? "cursor-pointer bg-brand-600 text-white hover:bg-brand-500"
              : "cursor-not-allowed bg-gray-100 text-gray-400 dark:bg-gray-800 dark:text-gray-600"
          }`}
        >
          {t("setup.continue")}
        </button>
      </div>
    </div>
  );
}

export default PermissionsSetup;
