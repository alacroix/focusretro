import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { emit } from "@tauri-apps/api/event";
import { getVersion } from "@tauri-apps/api/app";
import {
  getAutoswitchState,
  toggleAutoswitch,
  getGroupInviteState,
  toggleGroupInvite,
  getTradeState,
  toggleTrade,
  getPmState,
  togglePm,
  getAutoAcceptState,
  toggleAutoAccept,
  toggleShowDebug,
  HotkeyBinding,
  getHotkeys,
  setHotkey,
  getLanguage,
  setLanguage,
} from "../lib/commands";

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
      <div className="flex-1 mr-3">
        <div className="flex items-center gap-2">
          <span className="text-xs font-medium text-gray-200">{label}</span>
          {warn && (
            <span className="text-[10px] px-1.5 py-0.5 rounded bg-amber-900/50 text-amber-400">
              {warnLabel}
            </span>
          )}
        </div>
        {description && (
          <p className="text-[11px] text-gray-500 mt-0.5">{description}</p>
        )}
      </div>
      <button
        onClick={onToggle}
        className={`text-[11px] px-2.5 py-1 rounded-md transition-colors shrink-0 w-12 ${
          enabled
            ? "bg-emerald-600/20 text-emerald-400 hover:bg-emerald-600/30"
            : "bg-gray-800 text-gray-500 hover:bg-gray-700"
        }`}
      >
        {enabled ? onLabel : offLabel}
      </button>
    </div>
  );
}

const JS_KEY_TO_MAC_KEYCODE: Record<string, string> = {
  KeyA: "KeyA", KeyB: "KeyB", KeyC: "KeyC", KeyD: "KeyD", KeyE: "KeyE",
  KeyF: "KeyF", KeyG: "KeyG", KeyH: "KeyH", KeyI: "KeyI", KeyJ: "KeyJ",
  KeyK: "KeyK", KeyL: "KeyL", KeyM: "KeyM", KeyN: "KeyN", KeyO: "KeyO",
  KeyP: "KeyP", KeyQ: "KeyQ", KeyR: "KeyR", KeyS: "KeyS", KeyT: "KeyT",
  KeyU: "KeyU", KeyV: "KeyV", KeyW: "KeyW", KeyX: "KeyX", KeyY: "KeyY",
  KeyZ: "KeyZ",
  Digit0: "Digit0", Digit1: "Digit1", Digit2: "Digit2", Digit3: "Digit3",
  Digit4: "Digit4", Digit5: "Digit5", Digit6: "Digit6", Digit7: "Digit7",
  Digit8: "Digit8", Digit9: "Digit9",
  Space: "Space", Tab: "Tab",
  F1: "F1", F2: "F2", F3: "F3", F4: "F4", F5: "F5", F6: "F6",
  F7: "F7", F8: "F8", F9: "F9", F10: "F10", F11: "F11", F12: "F12",
};

const MOUSE_BUTTON_LABELS: Record<string, string> = {
  Mouse4: "Mouse 4",
  Mouse5: "Mouse 5",
};

function formatHotkeyLabel(hk: HotkeyBinding): string {
  const parts: string[] = [];
  if (hk.cmd) parts.push("Cmd");
  if (hk.ctrl) parts.push("Ctrl");
  if (hk.alt) parts.push("Alt");
  if (hk.shift) parts.push("Shift");
  parts.push(MOUSE_BUTTON_LABELS[hk.key] ?? hk.key.replace("Key", "").replace("Digit", ""));
  return parts.join("+");
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
}: {
  action: string;
  actionLabel: string;
  binding: HotkeyBinding | undefined;
  onRecord: (action: string) => void;
  recording: boolean;
  changeLabel: string;
  cancelLabel: string;
  pressKeyLabel: string;
}) {
  return (
    <div className="flex items-center justify-between py-2">
      <span className="text-xs text-gray-300">{actionLabel}</span>
      <div className="flex items-center gap-2">
        {recording ? (
          <span className="text-[11px] text-indigo-400 animate-pulse">
            {pressKeyLabel}
          </span>
        ) : (
          <kbd className="px-1.5 py-0.5 bg-gray-800 rounded text-[11px] text-gray-400 font-mono">
            {binding ? formatHotkeyLabel(binding) : "—"}
          </kbd>
        )}
        <button
          onClick={() => onRecord(action)}
          className={`text-[11px] px-2 py-1 rounded-md transition-colors ${
            recording
              ? "bg-indigo-600/20 text-indigo-400"
              : "bg-gray-800 text-gray-500 hover:bg-gray-700"
          }`}
        >
          {recording ? cancelLabel : changeLabel}
        </button>
      </div>
    </div>
  );
}

const KONAMI = [
  "ArrowUp", "ArrowUp", "ArrowDown", "ArrowDown",
  "ArrowLeft", "ArrowRight", "ArrowLeft", "ArrowRight",
  "b", "a",
];

function Settings({ showDebug, onToggleDebug }: { showDebug: boolean; onToggleDebug: (v: boolean) => void }) {
  const { t, i18n } = useTranslation();
  const [autoswitch, setAutoswitch] = useState(true);
  const [groupInvite, setGroupInvite] = useState(true);
  const [trade, setTrade] = useState(true);
  const [pm, setPm] = useState(true);
  const [autoAccept, setAutoAccept] = useState(false);
  const [hotkeys, setHotkeys] = useState<HotkeyBinding[]>([]);
  const [recordingAction, setRecordingAction] = useState<string | null>(null);
  const [language, setLang] = useState("en");
  const [version, setVersion] = useState("");
  const [unlocked, setUnlocked] = useState(false);
  const konamiProgress = useState<number>(0);

  useEffect(() => {
    getAutoswitchState().then(setAutoswitch);
    getGroupInviteState().then(setGroupInvite);
    getTradeState().then(setTrade);
    getPmState().then(setPm);
    getAutoAcceptState().then(setAutoAccept);
    getHotkeys().then(setHotkeys);
    getLanguage().then(setLang);
    getVersion().then(setVersion);
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
        emit("hotkeys-updated", newHotkeys);
      });
      setRecordingAction(null);
    };

    const keyHandler = (e: KeyboardEvent) => {
      e.preventDefault();
      e.stopPropagation();

      if (["Meta", "Alt", "Shift", "Control"].includes(e.key)) return;

      const code = e.code;
      if (!JS_KEY_TO_MAC_KEYCODE[code]) return;

      const isFnOrSpecial = /^(F\d+|Tab|Space)$/.test(code);
      if (!e.metaKey && !e.altKey && !e.shiftKey && !e.ctrlKey && !isFnOrSpecial) return;

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
  ];

  return (
    <div>
      <h2 className="text-sm font-medium text-gray-400 uppercase tracking-wider mb-2">
        {t("settings.title")}
      </h2>

      <div className="divide-y divide-gray-800/50">
        <ToggleRow
          label={t("settings.autoswitch")}
          description={t("settings.autoswitch_desc")}
          enabled={autoswitch}
          onToggle={async () => setAutoswitch(await toggleAutoswitch())}
          onLabel={t("settings.on")}
          offLabel={t("settings.off")}
        />
        <ToggleRow
          label={t("settings.group_invite")}
          description={t("settings.group_invite_desc")}
          enabled={groupInvite}
          onToggle={async () => setGroupInvite(await toggleGroupInvite())}
          onLabel={t("settings.on")}
          offLabel={t("settings.off")}
        />
        <ToggleRow
          label={t("settings.trade")}
          description={t("settings.trade_desc")}
          enabled={trade}
          onToggle={async () => setTrade(await toggleTrade())}
          onLabel={t("settings.on")}
          offLabel={t("settings.off")}
        />
        <ToggleRow
          label={t("settings.pm")}
          description={t("settings.pm_desc")}
          enabled={pm}
          onToggle={async () => setPm(await togglePm())}
          onLabel={t("settings.on")}
          offLabel={t("settings.off")}
        />
      </div>

      {unlocked && (
        <>
          <h3 className="text-sm font-medium text-gray-400 uppercase tracking-wider mt-5 mb-2">
            {t("settings.experimental")}
          </h3>
          <p className="text-xs text-amber-400/80 mb-3">{t("settings.experimental_warning")}</p>
          <div className="divide-y divide-gray-800/50">
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

      <h3 className="text-sm font-medium text-gray-400 uppercase tracking-wider mt-5 mb-2">
        {t("hotkeys.title")}
      </h3>

      <div className="divide-y divide-gray-800/50">
        {hotkeyActions.map(({ action, label }) => (
          <HotkeyRow
            key={action}
            action={action}
            actionLabel={label}
            binding={hotkeys.find((h) => h.action === action)}
            recording={recordingAction === action}
            onRecord={(a) =>
              setRecordingAction(recordingAction === a ? null : a)
            }
            changeLabel={t("hotkeys.change")}
            cancelLabel={t("hotkeys.cancel")}
            pressKeyLabel={t("hotkeys.press_key")}
          />
        ))}
      </div>

      <h3 className="text-sm font-medium text-gray-400 uppercase tracking-wider mt-5 mb-2">
        {t("language.title")}
      </h3>

      <select
        value={language}
        onChange={(e) => handleLanguageChange(e.target.value)}
        className="text-xs bg-transparent text-gray-300 focus:outline-none cursor-pointer"
      >
        <option value="en">🇬🇧 {t("language.en")}</option>
        <option value="fr">🇫🇷 {t("language.fr")}</option>
        <option value="es">🇪🇸 {t("language.es")}</option>
      </select>

      <p className="mt-6 text-center text-[11px] text-gray-600">
        FocusRetro v{version}
      </p>
    </div>
  );
}

export default Settings;
