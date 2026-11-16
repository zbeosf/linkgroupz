# Installing and Setting Up LinkGroupz

**English** | [한국어](INSTALL.ko.md)

LinkGroupz keeps the web pages you open every day in groups and opens a whole group in Chrome, Edge or Whale at once from the menu bar (the tray on Windows). Each group has its own browser (Chrome by default).
The illustrated guide is in the app: menu bar icon → **Guide** → **Open Guide** (the same as `사용법 · Guide/en/index.html` in the macOS DMG).

Requirements

- macOS 13 (Ventura) or later, **Apple Silicon (M1 or later) Macs only** — it doesn't run on Intel Macs. Or Windows 10 · 11 (64-bit)
- Google Chrome, Microsoft Edge or Naver Whale

## 1. Install

### macOS

1. Double-click `LinkGroupz-x.y.z.dmg`.
2. Drag **LinkGroupz** to the **Applications** folder.
3. Open LinkGroupz from the Applications folder. The first time, the welcome window and the manager open, and a three-cards icon appears in the menu bar (there's a Dock icon too, which you can turn off in Settings). After that, the menu bar window opens when you start the app.

#### If macOS says "LinkGroupz" Not Opened

This warning appears because the app isn't notarized by Apple. **Don't click "Move to Trash" — click "Done"**, then allow it once:

1. Open **System Settings → Privacy & Security**.
2. In the **Security** section near the bottom, click **Open Anyway** next to *"LinkGroupz" was blocked to protect your Mac.* (It only shows for about an hour after the warning; if you can't see it, open the app once more.)
3. Enter your Mac password and click **Open Anyway** in the next window.

If you use Terminal, this does the same:

```bash
xattr -dr com.apple.quarantine /Applications/LinkGroupz.app
```

### Windows

1. Run `LinkGroupz-x.y.z-setup.exe`. It installs for your account only, without admin rights.
2. If **Windows protected your PC** appears, click **More info** → **Run anyway** (it appears because the installer isn't signed).
3. Open LinkGroupz when the installation finishes. The first time, the welcome window and the manager open, and a three-cards icon appears in the tray at the right of the taskbar. After that, the tray window opens when you start the app. If you can't see the icon, click ∧ to find it among the hidden icons.

## 2. Set up

The first time you open it, the manager and the **Welcome to LinkGroupz** window appear. You can open the welcome window again from the menu bar (tray) icon → **Guide**.

To start it automatically after a restart, turn on menu bar (tray) icon → **Settings** → **Open at Login**.

To open the menu bar (tray) window from the keyboard, click **None** under **Settings** → **Shortcut** and press the shortcut you want (e.g. `⌥Space`, `Ctrl+Alt+K`). It works from any app, and you can search right away.

The app is in English or Korean. It follows your system language; change it in **Settings** → **Language**.

### Install the extension

With the extension, groups open as named, colored tab groups and you can import open tabs and tab groups. Install it in each browser you use (Chrome · Edge · Whale).

- **macOS**: recommended. You can import open tabs without it (without tab group information), but with it tabs are imported without touching the browser window.
- **Windows**: needed to import open tabs and tab groups.

1. Enter `chrome://extensions` in Chrome's address bar
2. Turn on **Developer mode** at the top right
3. **Load unpacked** → choose the folder below (the welcome window's **Show Extension Folder** button opens it)
   - macOS: `~/Library/Application Support/LinkGroupz/chrome-extension` (in the folder picker you can press `⌘⇧G` and paste the path)
   - Windows: `%APPDATA%\LinkGroupz\chrome-extension`
4. You're done when the welcome window shows **Extension installed**

Edge: `edge://extensions` → turn on **Developer mode** → **Load unpacked** → choose the same folder.

Whale: `whale://extensions` → turn on **Developer mode** → **Load unpacked extension** → choose the same folder.

### Allow these when macOS asks

| Prompt | When | Click |
|---|---|---|
| wants access to control "Google Chrome" (or "Microsoft Edge" · "Whale") | The first time you import open tabs from that browser (not asked if the LinkGroupz extension already sent the tab list) | Allow |
| Keychain access to "LinkGroupz Preview Sign-in Cookies" | When you open a preview again after signing in to one, or click **Check Saved Sign-ins** in Settings (it may ask again after an update) | Enter your Mac login password → **Always Allow** |

## 3. Update and uninstall

- **Automatic updates**: available in builds that include an update address. The app checks for a new version every hour and asks in the update window (**Update Now / Later**). When you update, a progress bar fills and the app restarts. You can also check with ↻ in Settings → **Updates**. Downloaded files are installed only after their signature is checked. Turn automatic checks off in Settings → **Updates** → **Check for updates automatically**. After an update, click reload (⟳) once on LinkGroupz Tab Groups on the extensions page (`chrome://extensions` · `edge://extensions` · `whale://extensions`) of each browser that has the extension.
- **Manual update**: copy LinkGroupz from the new DMG over the one in Applications (macOS), or run the new installer (Windows). Then reload LinkGroupz Tab Groups on the extensions page of each browser that has it. Your data and the Open at Login setting stay.
- **Uninstall**
  - macOS: menu bar icon → **Quit** → move LinkGroupz in Applications to the Trash → remove the extension from Chrome · Edge · Whale. To delete your data too, delete the `~/Library/Application Support/LinkGroupz` folder and the keychain item "LinkGroupz Preview Sign-in Cookies".
  - Windows: tray icon → **Quit** → Settings → Apps → Installed apps → uninstall LinkGroupz → remove the extension from Chrome · Edge · Whale. To delete your data too, delete the `%APPDATA%\LinkGroupz` folder.

## Where data is stored

The data folder is `~/Library/Application Support/LinkGroupz` on macOS and `%APPDATA%\LinkGroupz` on Windows.

| What | Where |
|---|---|
| Groups · pages · archive | `data.json` in the data folder |
| Settings | `settings.json` in the data folder |
| Chrome · Edge · Whale extension | `chrome-extension` in the data folder |
| Group icon images | `icons` in the data folder |
| Site icons | `favicons` in the data folder |
| Preview sign-in cookies | macOS: keychain item "LinkGroupz Preview Sign-in Cookies". Windows: `preview-profile` in the data folder (encrypted by WebView2) |

Copying `data.json` makes a backup, and putting it in the same place on another computer moves your groups (with the app quit).
The original LinkGroups `data.json` (`~/Library/Application Support/LinkGroups/data.json`) uses the same format, so copying it into the LinkGroupz data folder works as is.

## Open source licenses

Copyright and license notices for third-party code in LinkGroupz are in Settings → **Open Source Licenses** → **View**.
