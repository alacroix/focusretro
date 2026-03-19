import { invoke } from "@tauri-apps/api/core";

export interface GameWindow {
  character_name: string;
  window_id: number;
  pid: number;
  title: string;
}

export interface AccountView {
  character_name: string;
  window_id: number;
  pid: number;
  title: string;
  color: string | null;
  icon_path: string | null;
  is_principal: boolean;
  is_current: boolean;
  position: number;
}

export interface PermissionStatus {
  accessibility: boolean;
  screen_recording: boolean;
  input_monitoring: boolean;
}

export interface StoredMessage {
  receiver: string;
  sender: string;
  message: string;
  timestamp: number;
}

export function listAccounts(): Promise<AccountView[]> {
  return invoke("list_accounts");
}

export function refreshAccounts(): Promise<AccountView[]> {
  return invoke("refresh_accounts");
}

export function toggleAutoswitch(): Promise<boolean> {
  return invoke("toggle_autoswitch");
}

export function getAutoswitchState(): Promise<boolean> {
  return invoke("get_autoswitch_state");
}

export function focusAccount(name: string): Promise<void> {
  return invoke("focus_account", { name });
}

export function focusNextAccount(): Promise<string | null> {
  return invoke("focus_next_account");
}

export function focusPrevAccount(): Promise<string | null> {
  return invoke("focus_prev_account");
}

export function focusPrincipal(): Promise<string | null> {
  return invoke("focus_principal");
}

export function checkPermissions(): Promise<PermissionStatus> {
  return invoke("check_permissions");
}

export function requestScreenRecording(): Promise<void> {
  return invoke("request_screen_recording");
}

export function requestInputMonitoring(): Promise<void> {
  return invoke("request_input_monitoring");
}

export function openSettings(section: "accessibility" | "screen_recording" | "input_monitoring"): Promise<void> {
  return invoke("open_settings", { section });
}

export function toggleGroupInvite(): Promise<boolean> {
  return invoke("toggle_group_invite");
}

export function getGroupInviteState(): Promise<boolean> {
  return invoke("get_group_invite_state");
}

export function toggleTrade(): Promise<boolean> {
  return invoke("toggle_trade");
}

export function getTradeState(): Promise<boolean> {
  return invoke("get_trade_state");
}

export function togglePm(): Promise<boolean> {
  return invoke("toggle_pm");
}

export function getPmState(): Promise<boolean> {
  return invoke("get_pm_state");
}

export function getMessages(): Promise<StoredMessage[]> {
  return invoke("get_messages");
}

export function clearMessages(): Promise<void> {
  return invoke("clear_messages");
}

export function toggleAutoAccept(): Promise<boolean> {
  return invoke("toggle_auto_accept");
}

export function getAutoAcceptState(): Promise<boolean> {
  return invoke("get_auto_accept_state");
}


export function reorderAccount(name: string, newPosition: number): Promise<AccountView[]> {
  return invoke("reorder_account", { name, newPosition });
}

export function setPrincipal(name: string): Promise<AccountView[]> {
  return invoke("set_principal", { name });
}

export function updateAccountProfile(
  name: string,
  color: string | null,
  iconPath: string | null
): Promise<AccountView[]> {
  return invoke("update_account_profile", {
    name,
    color,
    iconPath,
  });
}

export function getProfiles(): Promise<AccountView[]> {
  return invoke("get_profiles");
}

export interface HotkeyBinding {
  action: string;
  key: string;
  cmd: boolean;
  alt: boolean;
  shift: boolean;
  ctrl: boolean;
}

export function getHotkeys(): Promise<HotkeyBinding[]> {
  return invoke("get_hotkeys");
}

export function setHotkey(
  action: string,
  key: string,
  cmd: boolean,
  alt: boolean,
  shift: boolean,
  ctrl: boolean
): Promise<HotkeyBinding[]> {
  return invoke("set_hotkey", { action, key, cmd, alt, shift, ctrl });
}

export function resetHotkeys(): Promise<HotkeyBinding[]> {
  return invoke("reset_hotkeys");
}

export function getLanguage(): Promise<string> {
  return invoke("get_language");
}

export function setLanguage(lang: string): Promise<void> {
  return invoke("set_language", { lang });
}

export interface TraceEntry {
  event_type: string;
  character_name: string;
  t_notification_ms: number;
  t_parsed_ms: number;
  t_focus_triggered_ms: number;
  t_focus_done_ms: number;
}

export function getTraces(): Promise<TraceEntry[]> {
  return invoke("get_traces");
}

export function clearTraces(): Promise<void> {
  return invoke("clear_traces");
}

export function getNotifMode(): Promise<string> {
  return invoke("get_notif_mode");
}

export function getShowDebug(): Promise<boolean> {
  return invoke("get_show_debug");
}

export function toggleShowDebug(): Promise<boolean> {
  return invoke("toggle_show_debug");
}

export function getTheme(): Promise<string> {
  return invoke("get_theme");
}

export function setTheme(theme: string): Promise<void> {
  return invoke("set_theme", { theme });
}

export function getAvailableLayouts(): Promise<string[]> {
  return invoke("get_available_layouts");
}

export function applyLayout(layout: string): Promise<void> {
  return invoke("apply_layout", { layout });
}

export function showRadial(): Promise<void> {
  return invoke("show_radial");
}

export function hideRadial(): Promise<void> {
  return invoke("hide_radial");
}

export function getUpdateConsent(): Promise<boolean | null> {
  return invoke("get_update_consent");
}

export function setUpdateConsent(consent: boolean): Promise<void> {
  return invoke("set_update_consent", { consent });
}

export function getCloseTotray(): Promise<boolean> {
  return invoke("get_close_to_tray");
}

export function setCloseTotray(value: boolean): Promise<void> {
  return invoke("set_close_to_tray", { value });
}
