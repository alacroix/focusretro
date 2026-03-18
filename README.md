# FocusRetro

<p align="center">
  <img src="src-tauri/icons/128x128.png" width="96" alt="FocusRetro logo" />
</p>

A lightweight desktop app that auto-focuses Dofus Retro game windows when it's your character's turn in combat, and lets you switch accounts via a radial overlay. Built for multi-account players.

## How it works

1. Dofus Retro sends a system notification when a game event occurs (turn start, group invite, trade request, private message)
2. FocusRetro intercepts the notification banner via macOS Accessibility APIs
3. The character name is extracted and matched to the corresponding game window
4. The game window is brought to the foreground automatically

No game client modification is needed.

## Features

- **Auto-focus on turn**: Switches to the correct game window when your turn starts
- **Group invite handling**: Focuses the recipient's window when one of your characters sends a group invite (restricted to known accounts)
- **Trade request handling**: Focuses the recipient's window when one of your characters sends a trade request (restricted to known accounts)
- **Auto-accept**: Optionally simulates pressing Enter to accept group invites and trades (off by default)
- **Private messages**: Captures incoming PMs and displays them in a dedicated Messages tab with HTML item links cleaned up
- **Account detection**: Detects all open Dofus Retro windows by parsing window titles
- **Account management**: Reorder accounts via drag & drop, assign icons and colors, designate a principal account
- **Radial character selector**: Hold a configurable hotkey to open a radial overlay centered on the cursor — move the mouse to the desired character slice and release to focus that account. Works on both macOS and Windows
- **Configurable global hotkeys**: Default bindings — customizable in Settings:
  - `F1` — Focus previous account
  - `F2` — Focus next account
  - `F3` — Focus principal account
  - Arrow keys also supported for previous/next navigation
- **Hotkeys work everywhere**: Uses low-level event hooks (`CGEventTap` on macOS, `WH_KEYBOARD_LL` on Windows) so hotkeys fire even when Dofus (Wine) has focus
- **System tray**: Dynamic icon (active/paused), shows principal account name, account count, toggle autoswitch, show/hide window, quit
- **Translations**: English, French, Spanish — auto-detects system language on first launch
- **Persistent settings**: All preferences (toggles, hotkeys, language, account profiles/order) saved to `~/.focusretro/config.json`
- **Independent toggles**: Each feature (autoswitch, group invite, trade, PM, auto-accept) can be enabled/disabled independently

## Requirements

### macOS

- macOS 12+ (Monterey or later)
- **Accessibility permission** must be granted:
  - System Settings → Privacy & Security → Accessibility → Enable FocusRetro

### Windows

- Windows 10 1903+ or Windows 11
- **Notification access** must be granted when prompted on first launch

### Build tools

- [Rust](https://rustup.rs/) 1.77.2+
- [Node.js](https://nodejs.org/) 18+
- npm 8+

## Build from source

```bash
# Clone the repository
git clone https://github.com/alacroix/focusretro.git
cd focusretro

# Install dependencies
npm install

# Run in development mode
npm run tauri dev

# Build for production
npm run tauri build
```

The built application will be in `src-tauri/target/release/bundle/`.

## Permissions

### macOS

FocusRetro requires **Accessibility** permission. This is used to:

1. **Read notification banners** — An `AXObserver` watches the NotificationCenter process for new notification windows and reads their text content
2. **Focus game windows** — `AXUIElement` APIs raise specific Dofus Retro windows
3. **Simulate keypresses** — `CGEvent` APIs handle auto-accept (Enter key) and global hotkey interception
4. **Global hotkeys** — A `CGEventTap` listens for keyboard events system-wide so hotkeys work even when another app is focused

When you first launch, you'll be prompted to grant Accessibility permission. If not, go to:

> System Settings → Privacy & Security → Accessibility

### Windows

FocusRetro requires **Notification access** to read toast notifications from Dofus Retro. You will be prompted on first launch via the `UserNotificationListener` API. No other special permissions are needed — window focus and key simulation use standard Win32 APIs.

## Architecture

```
src-tauri/src/
├── lib.rs                  # App setup, tray icon, tray menu
├── commands.rs             # Tauri IPC commands
├── state.rs                # Shared app state (toggles, accounts, messages, hotkeys, language, persistence)
├── radial.rs               # Radial overlay geometry (segment hit-test, selection resolution, focus dispatch)
├── core/
│   ├── autoswitch.rs       # Main autoswitch controller
│   ├── accounts.rs         # Account detection
│   └── parser.rs           # Notification parser (turn, invite, trade, PM)
└── platform/
    ├── mod.rs              # Platform traits (WindowManager, NotificationListener)
    ├── macos/
    │   ├── hotkeys.rs      # CGEventTap global hotkey listener
    │   ├── window.rs       # CGWindowList + AXUIElement + CGEvent
    │   ├── notifications.rs # AXObserver on NotificationCenter
    │   └── permissions.rs  # Accessibility permission checks
    └── windows/
    │   ├── hotkeys.rs      # WH_KEYBOARD_LL global hotkey listener
    │   ├── window.rs       # EnumWindows + HWND-direct focus (AttachThreadInput/SetActiveWindow) + SendInput
    │   └── notifications.rs # WinRT UserNotificationListener

src/
├── i18n.ts                 # i18next initialization (en/fr/es)
├── locales/                # Translation JSON files
├── App.tsx                 # Main layout with tabs
├── components/
│   ├── AccountList.tsx     # Account list with drag & drop, icons, colors
│   ├── MessageList.tsx     # PM display
│   ├── RadialSelector.tsx  # Radial account picker overlay (SVG wheel)
│   └── Settings.tsx        # Toggles, hotkey config, language selector
└── lib/
    └── commands.ts         # Typed Tauri invoke wrappers
```

## Supported notification formats

| Event | Body pattern | Action |
|-------|-------------|--------|
| Turn | FR: `C'est à <name> de jouer` / EN: `<name> 's turn to play` / ES: `le toca jugar a <name>` | Focus the named character's window |
| Group invite | FR: `<name> t'invite à rejoindre son groupe` / EN: `You are invited to join <name>'s group` / ES: `<name> te invita a unirte a su grupo` | Focus the receiver's window (inviter must be a known account) |
| Trade | FR: `<name> te propose de faire un échange` / EN: `<name> offers a trade` / ES: `<name> te propone realizar un intercambio` | Focus the receiver's window (requester must be a known account) |
| Private message | FR: `de <name> : <msg>` / EN: `from <name> : <msg>` / ES: `desde <name> : <msg>` | Store and display in Messages tab (no focus) |

## Supported platforms

| Platform | Status |
|----------|--------|
| macOS    | Supported |
| Windows  | Supported |

## Code signing (macOS)

macOS binaries are signed with an ad-hoc identity (`-`). They are **not notarized**, so Gatekeeper will block the app when downloaded from the internet. To run it:

```bash
xattr -d com.apple.quarantine /Applications/Focus\ Retro.app
```

Or right-click the app → Open → Open anyway.

TODO: set up proper Apple Developer signing + notarization for a smoother install experience.

## Code signing (Windows)

Windows binaries are currently unsigned. SmartScreen will warn on first install — click "More info → Run anyway".

TODO: set up signing via [Azure Artifact Signing](https://learn.microsoft.com/en-us/azure/trusted-signing/) (~$10/month, requires registered organization) or [SignPath Foundation](https://signpath.org/) (free for OSS). See `.github/workflows/release.yml` and `src-tauri/tauri.conf.json` — the `signCommand` config is already in place, just needs credentials.

## Security

Only download FocusRetro from the official repository: **[github.com/alacroix/focusretro](https://github.com/alacroix/focusretro/releases/latest)**.
Third-party sites have been known to redistribute modified Dofus community tools containing malware.

Every release asset is attested by GitHub Actions. You can verify a binary before running it:

```bash
gh attestation verify <file> --repo alacroix/focusretro
```

Example:
```bash
gh attestation verify Focus.Retro_0.3.0_aarch64.dmg --repo alacroix/focusretro
```

A valid attestation confirms the file was built automatically from source — no human intervention.

## Disclaimer

FocusRetro is an open-source utility for players of Dofus Retro.

This project is not affiliated with or endorsed by Ankama.
Dofus is a trademark of Ankama.

## License

MIT
