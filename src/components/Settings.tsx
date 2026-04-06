import { getVersion } from "@tauri-apps/api/app";
import { useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";

import {
  getAutoswitchState,
  toggleAutoswitch,
  getGroupInviteState,
  toggleGroupInvite,
  getTradeState,
  toggleTrade,
  togglePm,
  getAutoAcceptState,
  toggleAutoAccept,
  toggleShowDebug,
  HotkeyBinding,
  getHotkeys,
  setHotkey,
  setLanguage,
  resetHotkeys,
  getCloseTotray,
  setCloseTotray as setCloseTotrayCmd,
  toggleTaskbarUngroup,
  setIconStyle as setIconStyleCmd,
  toggleWorkshopInvite,
  getWorkshopInviteState,
} from "../lib/commands";
import { renderAccountIcon } from "../lib/taskbarIcon";

function ToggleRow({
  label,
  description,
  enabled,
  onToggle,
  warn,
  warnLabel,
  onLabel,
  offLabel,
}: {
  label: string;
  description?: string;
  enabled: boolean;
  onToggle: () => void;
  warn?: boolean;
  warnLabel?: string;
  onLabel: string;
  offLabel: string;
}) {
  return (
    <div className="flex items-center justify-between py-2">
      <div className="mr-3 flex-1">
        <div className="flex items-center gap-2">
          <span className="text-xs font-medium text-gray-800 dark:text-gray-200">{label}</span>
          {warn && (
            <span className="rounded bg-amber-50 px-1.5 py-0.5 text-[10px] text-amber-600 dark:bg-amber-900/50 dark:text-amber-400">
              {warnLabel}
            </span>
          )}
        </div>
        {description && <p className="mt-0.5 text-[11px] text-gray-500">{description}</p>}
      </div>
      <button
        onClick={onToggle}
        className={`w-12 shrink-0 cursor-pointer rounded-md px-2.5 py-1 text-[11px] transition-colors ${
          enabled
            ? "bg-emerald-50 text-emerald-600 hover:bg-emerald-100 dark:bg-emerald-600/20 dark:text-emerald-400 dark:hover:bg-emerald-600/30"
            : "bg-gray-100 text-gray-500 hover:bg-gray-200 dark:bg-gray-800 dark:text-gray-500 dark:hover:bg-gray-700"
        }`}
      >
        {enabled ? onLabel : offLabel}
      </button>
    </div>
  );
}

const JS_KEY_TO_MAC_KEYCODE: Record<string, string> = {
  KeyA: "KeyA",
  KeyB: "KeyB",
  KeyC: "KeyC",
  KeyD: "KeyD",
  KeyE: "KeyE",
  KeyF: "KeyF",
  KeyG: "KeyG",
  KeyH: "KeyH",
  KeyI: "KeyI",
  KeyJ: "KeyJ",
  KeyK: "KeyK",
  KeyL: "KeyL",
  KeyM: "KeyM",
  KeyN: "KeyN",
  KeyO: "KeyO",
  KeyP: "KeyP",
  KeyQ: "KeyQ",
  KeyR: "KeyR",
  KeyS: "KeyS",
  KeyT: "KeyT",
  KeyU: "KeyU",
  KeyV: "KeyV",
  KeyW: "KeyW",
  KeyX: "KeyX",
  KeyY: "KeyY",
  KeyZ: "KeyZ",
  Digit0: "Digit0",
  Digit1: "Digit1",
  Digit2: "Digit2",
  Digit3: "Digit3",
  Digit4: "Digit4",
  Digit5: "Digit5",
  Digit6: "Digit6",
  Digit7: "Digit7",
  Digit8: "Digit8",
  Digit9: "Digit9",
  Space: "Space",
  Tab: "Tab",
  ArrowLeft: "ArrowLeft",
  ArrowRight: "ArrowRight",
  ArrowUp: "ArrowUp",
  ArrowDown: "ArrowDown",
  F1: "F1",
  F2: "F2",
  F3: "F3",
  F4: "F4",
  F5: "F5",
  F6: "F6",
  F7: "F7",
  F8: "F8",
  F9: "F9",
  F10: "F10",
  F11: "F11",
  F12: "F12",
  Numpad0: "Numpad0",
  Numpad1: "Numpad1",
  Numpad2: "Numpad2",
  Numpad3: "Numpad3",
  Numpad4: "Numpad4",
  Numpad5: "Numpad5",
  Numpad6: "Numpad6",
  Numpad7: "Numpad7",
  Numpad8: "Numpad8",
  Numpad9: "Numpad9",
  NumpadAdd: "NumpadAdd",
  NumpadSubtract: "NumpadSubtract",
  NumpadMultiply: "NumpadMultiply",
  NumpadDivide: "NumpadDivide",
  NumpadDecimal: "NumpadDecimal",
};

const MOUSE_BUTTON_LABELS: Record<string, string> = {
  Mouse4: "Mouse 4",
  Mouse5: "Mouse 5",
};

const ARROW_LABELS: Record<string, string> = {
  ArrowLeft: "←",
  ArrowRight: "→",
  ArrowUp: "↑",
  ArrowDown: "↓",
};

const NUMPAD_LABELS: Record<string, string> = {
  Numpad0: "Num 0",
  Numpad1: "Num 1",
  Numpad2: "Num 2",
  Numpad3: "Num 3",
  Numpad4: "Num 4",
  Numpad5: "Num 5",
  Numpad6: "Num 6",
  Numpad7: "Num 7",
  Numpad8: "Num 8",
  Numpad9: "Num 9",
  NumpadAdd: "Num +",
  NumpadSubtract: "Num -",
  NumpadMultiply: "Num *",
  NumpadDivide: "Num /",
  NumpadDecimal: "Num .",
};

function formatHotkeyLabel(hk: HotkeyBinding, layoutMap: Map<string, string>): string {
  const parts: string[] = [];
  if (hk.cmd) parts.push("Cmd");
  if (hk.ctrl) parts.push("Ctrl");
  if (hk.alt) parts.push("Alt");
  if (hk.shift) parts.push("Shift");
  parts.push(
    MOUSE_BUTTON_LABELS[hk.key] ??
      ARROW_LABELS[hk.key] ??
      NUMPAD_LABELS[hk.key] ??
      (hk.key.startsWith("Key")
        ? (layoutMap.get(hk.key)?.toUpperCase() ?? hk.key.replace("Key", ""))
        : hk.key.startsWith("Digit")
          ? hk.key.replace("Digit", "")
          : hk.key),
  );
  return parts.join(" + ");
}

function HotkeyRow({
  action,
  actionLabel,
  binding,
  onRecord,
  recording,
  changeLabel,
  cancelLabel,
  pressKeyLabel,
  layoutMap,
}: {
  action: string;
  actionLabel: string;
  binding: HotkeyBinding | undefined;
  onRecord: (action: string) => void;
  recording: boolean;
  changeLabel: string;
  cancelLabel: string;
  pressKeyLabel: string;
  layoutMap: Map<string, string>;
}) {
  return (
    <div className="flex items-center justify-between py-2">
      <span className="text-xs text-gray-700 dark:text-gray-300">{actionLabel}</span>
      <div className="flex items-center gap-2">
        {recording ? (
          <span className="animate-pulse text-[11px] text-brand-600 dark:text-brand-400">
            {pressKeyLabel}
          </span>
        ) : (
          <kbd className="rounded bg-gray-100 px-1.5 py-0.5 font-mono text-[11px] whitespace-nowrap text-gray-700 dark:bg-gray-800 dark:text-gray-400">
            {binding && binding.key ? formatHotkeyLabel(binding, layoutMap) : "—"}
          </kbd>
        )}
        <button
          onClick={() => onRecord(action)}
          className={`cursor-pointer rounded-md px-2 py-1 text-[11px] transition-colors ${
            recording
              ? "bg-brand-50 text-brand-700 dark:bg-brand-600/20 dark:text-brand-400"
              : "bg-gray-100 text-gray-500 hover:bg-gray-200 dark:bg-gray-800 dark:text-gray-500 dark:hover:bg-gray-700"
          }`}
        >
          {recording ? cancelLabel : changeLabel}
        </button>
      </div>
    </div>
  );
}

function ThemeSelector({ theme, onChange }: { theme: string; onChange: (t: string) => void }) {
  const { t } = useTranslation();
  const options = [
    {
      value: "system",
      label: t("settings.theme_system"),
      icon: (
        <svg
          width="13"
          height="13"
          viewBox="0 0 24 24"
          fill="none"
          stroke="currentColor"
          strokeWidth="2"
          strokeLinecap="round"
          strokeLinejoin="round"
        >
          <rect x="2" y="3" width="20" height="14" rx="2" />
          <path d="M8 21h8M12 17v4" />
        </svg>
      ),
    },
    {
      value: "light",
      label: t("settings.theme_light"),
      icon: (
        <svg
          width="13"
          height="13"
          viewBox="0 0 24 24"
          fill="none"
          stroke="currentColor"
          strokeWidth="2"
          strokeLinecap="round"
          strokeLinejoin="round"
        >
          <circle cx="12" cy="12" r="5" />
          <path d="M12 1v2M12 21v2M4.22 4.22l1.42 1.42M18.36 18.36l1.42 1.42M1 12h2M21 12h2M4.22 19.78l1.42-1.42M18.36 5.64l1.42-1.42" />
        </svg>
      ),
    },
    {
      value: "dark",
      label: t("settings.theme_dark"),
      icon: (
        <svg
          width="13"
          height="13"
          viewBox="0 0 24 24"
          fill="none"
          stroke="currentColor"
          strokeWidth="2"
          strokeLinecap="round"
          strokeLinejoin="round"
        >
          <path d="M21 12.79A9 9 0 1 1 11.21 3 7 7 0 0 0 21 12.79z" />
        </svg>
      ),
    },
  ];

  return (
    <div className="mt-1 flex gap-1">
      {options.map((opt) => (
        <button
          key={opt.value}
          onClick={() => onChange(opt.value)}
          title={opt.label}
          className={`flex cursor-pointer items-center gap-1.5 rounded-md px-2.5 py-1.5 text-[11px] transition-colors ${
            theme === opt.value
              ? "bg-brand-50 text-brand-700 dark:bg-brand-600/20 dark:text-brand-400"
              : "bg-gray-100 text-gray-500 hover:bg-gray-200 dark:bg-gray-800 dark:text-gray-500 dark:hover:bg-gray-700"
          }`}
        >
          {opt.icon}
          {opt.label}
        </button>
      ))}
    </div>
  );
}

function IconStylePreview({ mode }: { mode: "classic" | "portrait" }) {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  useEffect(() => {
    renderAccountIcon("90", "#ef4444", mode, false).then((rgba) => {
      const canvas = canvasRef.current;
      if (!canvas) return;
      const ctx = canvas.getContext("2d")!;
      const imageData = new ImageData(new Uint8ClampedArray(rgba), 24, 24);
      ctx.putImageData(imageData, 0, 0);
    });
  }, [mode]);
  return <canvas ref={canvasRef} width={24} height={24} className="h-6 w-6 rounded" />;
}

const KONAMI = [
  "ArrowUp",
  "ArrowUp",
  "ArrowDown",
  "ArrowDown",
  "ArrowLeft",
  "ArrowRight",
  "ArrowLeft",
  "ArrowRight",
  "b",
  "a",
];

function Settings({
  showDebug,
  onToggleDebug,
  pmEnabled,
  onTogglePm,
  theme,
  onThemeChange,
  updateConsent,
  onUpdateConsentChange,
  onCheckUpdate,
  taskbarUngroup,
  onToggleTaskbarUngroup,
  iconStyle,
  onIconStyleChange,
  onHotkeysChange,
  hotkeysFocusedOnly,
  onToggleHotkeysFocusedOnly,
  hotkeysConsume,
  onToggleHotkeysConsume,
}: {
  showDebug: boolean;
  onToggleDebug: (v: boolean) => void;
  pmEnabled: boolean;
  onTogglePm: (v: boolean) => void;
  theme: string;
  onThemeChange: (t: string) => void;
  updateConsent: boolean | null | undefined;
  onUpdateConsentChange: (consent: boolean) => void;
  onCheckUpdate: () => Promise<boolean>;
  taskbarUngroup: boolean;
  onToggleTaskbarUngroup: (v: boolean) => void;
  iconStyle: "classic" | "portrait";
  onIconStyleChange: (style: "classic" | "portrait") => void;
  onHotkeysChange?: (bindings: HotkeyBinding[]) => void;
  hotkeysFocusedOnly: boolean;
  onToggleHotkeysFocusedOnly: (v: boolean) => void;
  hotkeysConsume: boolean;
  onToggleHotkeysConsume: (v: boolean) => void;
}) {
  const { t, i18n } = useTranslation();
  const [autoswitch, setAutoswitch] = useState(true);
  const [groupInvite, setGroupInvite] = useState(true);
  const [trade, setTrade] = useState(true);
  const [workshop, setWorkshop] = useState(true);
  const [autoAccept, setAutoAccept] = useState(false);
  const [closeTotray, setCloseTotray] = useState(true);
  const [hotkeys, setHotkeys] = useState<HotkeyBinding[]>([]);
  const [recordingAction, setRecordingAction] = useState<string | null>(null);
  const [language, setLang] = useState(i18n.language);
  const [version, setVersion] = useState("");
  const [unlocked, setUnlocked] = useState(false);
  const [checkState, setCheckState] = useState<"idle" | "checking" | "up-to-date">("idle");
  const [layoutMap, setLayoutMap] = useState<Map<string, string>>(new Map());
  const checkTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const konamiProgress = useState<number>(0);

  useEffect(() => {
    getAutoswitchState().then(setAutoswitch);
    getGroupInviteState().then(setGroupInvite);
    getTradeState().then(setTrade);
    getWorkshopInviteState().then(setWorkshop);
    getAutoAcceptState().then(setAutoAccept);
    getCloseTotray().then(setCloseTotray);
    getHotkeys().then(setHotkeys);
    getVersion().then(setVersion);
    (
      navigator as Navigator & { keyboard?: { getLayoutMap(): Promise<Map<string, string>> } }
    ).keyboard
      ?.getLayoutMap()
      .then(setLayoutMap);
  }, []);

  useEffect(() => {
    return () => {
      if (checkTimerRef.current) clearTimeout(checkTimerRef.current);
    };
  }, []);

  useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      const next = konamiProgress[0] + 1;
      if (e.key === KONAMI[konamiProgress[0]]) {
        if (next === KONAMI.length) {
          setUnlocked((u) => !u);
          konamiProgress[1](0);
        } else {
          konamiProgress[1](next);
        }
      } else {
        konamiProgress[1](e.key === KONAMI[0] ? 1 : 0);
      }
    };
    window.addEventListener("keydown", handler);
    return () => window.removeEventListener("keydown", handler);
  }, [konamiProgress[0]]);

  useEffect(() => {
    if (!recordingAction) return;

    const save = (key: string, cmd: boolean, alt: boolean, shift: boolean, ctrl: boolean) => {
      setHotkey(recordingAction, key, cmd, alt, shift, ctrl).then((newHotkeys) => {
        setHotkeys(newHotkeys);
        onHotkeysChange?.(newHotkeys);
      });
      setRecordingAction(null);
    };

    const keyHandler = (e: KeyboardEvent) => {
      e.preventDefault();
      e.stopPropagation();

      if (["Meta", "Alt", "Shift", "Control"].includes(e.key)) return;

      const code = e.code;
      if (!JS_KEY_TO_MAC_KEYCODE[code]) return;

      save(JS_KEY_TO_MAC_KEYCODE[code], e.metaKey, e.altKey, e.shiftKey, e.ctrlKey);
    };

    const mouseHandler = (e: MouseEvent) => {
      // Only capture side buttons (Mouse4 = browser button 3, Mouse5 = browser button 4)
      if (e.button < 3) return;
      e.preventDefault();
      e.stopPropagation();
      save(e.button === 3 ? "Mouse4" : "Mouse5", e.metaKey, e.altKey, e.shiftKey, e.ctrlKey);
    };

    window.addEventListener("keydown", keyHandler, true);
    window.addEventListener("mousedown", mouseHandler, true);
    return () => {
      window.removeEventListener("keydown", keyHandler, true);
      window.removeEventListener("mousedown", mouseHandler, true);
    };
  }, [recordingAction]);

  const handleLanguageChange = async (lang: string) => {
    setLang(lang);
    i18n.changeLanguage(lang);
    await setLanguage(lang);
  };

  const hotkeyActions = [
    { action: "prev", label: t("hotkeys.prev") },
    { action: "next", label: t("hotkeys.next") },
    { action: "principal", label: t("hotkeys.principal") },
    { action: "radial", label: t("hotkeys.radial") },
  ];

  return (
    <div>
      <h2 className="mb-2 text-sm font-medium tracking-wider text-gray-500 uppercase dark:text-gray-400">
        {t("settings.title")}
      </h2>

      <div className="divide-y divide-gray-200 dark:divide-gray-800/50">
        <ToggleRow
          label={t("settings.autoswitch")}
          description={t("settings.autoswitch_desc")}
          enabled={autoswitch}
          onToggle={async () => setAutoswitch(await toggleAutoswitch())}
          onLabel={t("settings.on")}
          offLabel={t("settings.off")}
        />
        <div className="flex items-center justify-between py-3">
          <span className="text-xs font-medium text-gray-800 dark:text-gray-200">
            {t("settings.focus_interactions")}
          </span>
          <div className="flex gap-2">
            {(
              [
                {
                  key: "group",
                  ext: "png",
                  enabled: groupInvite,
                  toggle: () => toggleGroupInvite().then(setGroupInvite),
                  tooltip: t("settings.group_invite_tooltip"),
                },
                {
                  key: "trade",
                  ext: "svg",
                  enabled: trade,
                  toggle: () => toggleTrade().then(setTrade),
                  tooltip: t("settings.trade_tooltip"),
                },
                {
                  key: "workshop",
                  ext: "svg",
                  enabled: workshop,
                  toggle: () => toggleWorkshopInvite().then(setWorkshop),
                  tooltip: t("settings.workshop_invite_tooltip"),
                },
              ] as const
            ).map(({ key, ext, enabled, toggle, tooltip }) => (
              <button
                key={key}
                title={tooltip}
                onClick={toggle}
                className={`cursor-pointer rounded-lg p-1 transition-all ${
                  enabled ? "opacity-100" : "opacity-40 grayscale"
                }`}
              >
                <img
                  src={`/settings/${key}.${ext}`}
                  alt={tooltip}
                  className="h-6 w-6 object-contain"
                />
              </button>
            ))}
          </div>
        </div>
        {navigator.userAgent.includes("Windows NT") && (
          <ToggleRow
            label={t("settings.taskbar_ungroup")}
            description={t("settings.taskbar_ungroup_desc")}
            enabled={taskbarUngroup}
            onToggle={async () => onToggleTaskbarUngroup(await toggleTaskbarUngroup())}
            onLabel={t("settings.on")}
            offLabel={t("settings.off")}
          />
        )}
        {navigator.userAgent.includes("Windows NT") && taskbarUngroup && (
          <div className="py-2">
            <div className="mb-2">
              <span className="text-xs font-medium text-gray-800 dark:text-gray-200">
                {t("settings.icon_style")}
              </span>
              <p className="mt-0.5 text-[11px] text-gray-500">{t("settings.icon_style_desc")}</p>
            </div>
            <div className="flex gap-2 [&>button]:flex-1">
              {(["classic", "portrait"] as const).map((mode) => (
                <button
                  key={mode}
                  onClick={async () => {
                    await setIconStyleCmd(mode);
                    onIconStyleChange(mode);
                  }}
                  className={`flex cursor-pointer flex-row items-center gap-2 rounded-lg border-2 px-3 py-2 transition-colors ${
                    iconStyle === mode
                      ? "border-brand-500 bg-brand-50 dark:bg-gray-800"
                      : "border-gray-200 hover:border-gray-300 dark:border-gray-700 dark:hover:border-gray-600"
                  }`}
                >
                  <IconStylePreview mode={mode} />
                  <span className="text-xs text-gray-700 dark:text-gray-300">
                    {t(`settings.icon_style_${mode}`)}
                  </span>
                </button>
              ))}
            </div>
          </div>
        )}
        <ToggleRow
          label={t("settings.pm")}
          description={t("settings.pm_desc")}
          enabled={pmEnabled}
          onToggle={async () => onTogglePm(await togglePm())}
          onLabel={t("settings.on")}
          offLabel={t("settings.off")}
        />
        {import.meta.env.VITE_UPDATER !== "false" && (
          <ToggleRow
            label={t("settings.update_check")}
            description={t("settings.update_check_desc")}
            enabled={updateConsent === true}
            onToggle={() => onUpdateConsentChange(!(updateConsent === true))}
            onLabel={t("settings.on")}
            offLabel={t("settings.off")}
          />
        )}
      </div>

      {unlocked && (
        <>
          <h3 className="mt-5 mb-2 text-sm font-medium tracking-wider text-gray-500 uppercase dark:text-gray-400">
            {t("settings.experimental")}
          </h3>
          <p className="mb-3 text-xs text-amber-600 dark:text-amber-400/80">
            {t("settings.experimental_warning")}
          </p>
          <div className="divide-y divide-gray-200 dark:divide-gray-800/50">
            <ToggleRow
              label={t("settings.auto_accept")}
              description={t("settings.auto_accept_desc")}
              enabled={autoAccept}
              onToggle={async () => setAutoAccept(await toggleAutoAccept())}
              onLabel={t("settings.on")}
              offLabel={t("settings.off")}
            />
            <ToggleRow
              label={t("settings.show_debug")}
              description={t("settings.show_debug_desc")}
              enabled={showDebug}
              onToggle={async () => onToggleDebug(await toggleShowDebug())}
              onLabel={t("settings.on")}
              offLabel={t("settings.off")}
            />
          </div>
        </>
      )}

      <h3 className="mt-5 mb-3 text-sm font-medium tracking-wider text-gray-500 uppercase dark:text-gray-400">
        {t("settings.display")}
      </h3>

      <div className="divide-y divide-gray-200 dark:divide-gray-800/50">
        <div className="flex items-center justify-between py-2">
          <span className="text-xs font-medium text-gray-800 dark:text-gray-200">
            {t("language.title")}
          </span>
          <select
            value={language}
            onChange={(e) => handleLanguageChange(e.target.value)}
            className="cursor-pointer bg-transparent text-xs text-gray-700 focus:outline-none dark:text-gray-300"
          >
            <option value="en">🇬🇧 {t("language.en")}</option>
            <option value="fr">🇫🇷 {t("language.fr")}</option>
            <option value="es">🇪🇸 {t("language.es")}</option>
          </select>
        </div>
        <div className="flex items-center justify-between py-2">
          <span className="text-xs font-medium text-gray-800 dark:text-gray-200">
            {t("settings.theme")}
          </span>
          <ThemeSelector theme={theme} onChange={onThemeChange} />
        </div>
        <ToggleRow
          label={t("settings.close_to_tray")}
          enabled={closeTotray}
          onToggle={async () => {
            const v = !closeTotray;
            setCloseTotray(v);
            await setCloseTotrayCmd(v).catch(() => setCloseTotray(!v));
          }}
          onLabel={t("settings.on")}
          offLabel={t("settings.off")}
        />
      </div>

      <div className="mt-5 mb-2 flex items-center justify-between">
        <h3 className="text-sm font-medium tracking-wider text-gray-500 uppercase dark:text-gray-400">
          {t("hotkeys.title")}
        </h3>
        <button
          onClick={() => {
            resetHotkeys().then((newHotkeys) => {
              setHotkeys(newHotkeys);
              onHotkeysChange?.(newHotkeys);
            });
          }}
          className="cursor-pointer text-[11px] text-gray-400 transition-colors hover:text-gray-600 dark:text-gray-600 dark:hover:text-gray-400"
        >
          {t("hotkeys.reset")}
        </button>
      </div>

      <div className="overflow-hidden rounded-lg border border-gray-200 bg-gray-50 dark:border-gray-800 dark:bg-gray-900/60">
        <div className="divide-y divide-gray-200 px-3 py-1 dark:divide-gray-800">
          {hotkeyActions.map(({ action, label }) => (
            <HotkeyRow
              key={action}
              action={action}
              actionLabel={label}
              binding={hotkeys.find((h) => h.action === action)}
              recording={recordingAction === action}
              onRecord={(a) => setRecordingAction(recordingAction === a ? null : a)}
              changeLabel={t("hotkeys.change")}
              cancelLabel={t("hotkeys.cancel")}
              pressKeyLabel={t("hotkeys.press_key")}
              layoutMap={layoutMap}
            />
          ))}
        </div>
      </div>

      <div className="divide-y divide-gray-200 dark:divide-gray-800/50">
        <ToggleRow
          label={t("hotkeys.focused_only")}
          enabled={hotkeysFocusedOnly}
          onToggle={() => onToggleHotkeysFocusedOnly(!hotkeysFocusedOnly)}
          onLabel={t("settings.on")}
          offLabel={t("settings.off")}
        />
        <ToggleRow
          label={t("hotkeys.consume")}
          enabled={hotkeysConsume}
          onToggle={() => onToggleHotkeysConsume(!hotkeysConsume)}
          onLabel={t("settings.on")}
          offLabel={t("settings.off")}
        />
      </div>

      <div className="mt-6 flex flex-col items-center gap-1 text-[11px] text-gray-400 dark:text-gray-600">
        {import.meta.env.VITE_UPDATER !== "false" &&
          (checkState === "checking" ? (
            <span className="animate-pulse">{t("settings.checking")}</span>
          ) : checkState === "up-to-date" ? (
            <span>{t("settings.up_to_date")}</span>
          ) : (
            <button
              onClick={() => {
                if (checkTimerRef.current) clearTimeout(checkTimerRef.current);
                setCheckState("checking");
                onCheckUpdate()
                  .then((hasUpdate) => {
                    if (hasUpdate) {
                      setCheckState("idle");
                    } else {
                      setCheckState("up-to-date");
                      checkTimerRef.current = setTimeout(() => setCheckState("idle"), 3000);
                    }
                  })
                  .catch(() => setCheckState("idle"));
              }}
              className="cursor-pointer text-gray-400 underline underline-offset-2 transition-colors hover:text-gray-600 dark:text-gray-600 dark:hover:text-gray-400"
            >
              {t("settings.check_now")}
            </button>
          ))}
        <span className="flex items-center gap-1.5">
          FocusRetro v{version}
          {import.meta.env.VITE_UPDATER === "false" && (
            <>
              <span>— offline build</span>
              <svg
                width="11"
                height="11"
                viewBox="0 0 24 24"
                fill="none"
                stroke="currentColor"
                strokeWidth="2"
                strokeLinecap="round"
                strokeLinejoin="round"
              >
                <path d="M18 10h-1.26A8 8 0 1 0 9 20h9a5 5 0 0 0 0-10z" />
                <line x1="2" y1="2" x2="22" y2="22" />
              </svg>
            </>
          )}
        </span>
      </div>
    </div>
  );
}

export default Settings;
