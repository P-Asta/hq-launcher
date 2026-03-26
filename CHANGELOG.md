## Version (YYYY-MM-DD)
- Content

## 1.7.3 (2026-03-26)
- Fixed an issue where the “Cancel” button had significant lag.
- Added a progress indicator when downloading and extracting large files so you can tell if the app has frozen.
- Optimized the process for saving Thunderstore information. (I hope this works well.)
- Fixed an issue where English text was occasionally not displayed due to fontpatcher (a mod that displays unsupported languages).

## 1.7.1 (2026-03-25)
- Add more Practice mods, including `CoordinateForEasterEggs`, `Yukieji_UnityExplorer`, `LCSeedPicker`, and `IntroTweaks` plus the required hidden helper dependencies on supported versions.
- Add version deletion from the version picker with a right-click context menu, confirmation, and live delete progress.
- Dock the config editor beside the mod list and make the split view resizable.
- Rework Practice Modes to use the current preset-specific, version-aware practice mod set.
- Hide internal practice helper mods from the main UI and preserve user-disabled practice mods instead of forcing them back on.
- Make mod update checks respect the selected run mode so preset/practice-only mods do not create false update results.
- Fix BepInEx manifest parsing for BOM-encoded files and avoid partial manifest reads during extraction, reducing HookGenPatcher manifest warnings and scan stalls.
- Improve launcher styling for SUIT Variable with a shared panel outline color token, and increase the default window size for the denser layout.

## 1.6.3 (2026-03-22)
- Avoid prompting to install DepotDownloader again when it is already installed.

## 1.6.2 (2026-03-20)
- Set `LockMoons = true` for `Wesley's Run`, and `LockMoons = false` for `Wesley's Practice` and `Wesley's SMHQ`.

## 1.6.1 (2026-03-20)
- Automatically set `JacobG5.ReverbTriggerFix.cfg` to use `triggerOnEnter = true` when `ReverbTriggerFix` is installed.
- Automatically add `DungeonKeyItem` to HQoL's `Dont store list` when it is missing.
- Add optional `tag_constraints` support in the mod manifest so tagged mods can use per-pack version caps.


## 1.6.0 (2026-03-18)
- MayB fix DepotDownloader download bug 

## 1.5.2 (2026-03-17)
- Show installed mod icons from each mod folder's `icon.png` in the mod list.
- Show installed mod descriptions in the mod list and config header.
- Add a right-click context menu on mods with an option to open the selected mod folder.
- Add a File menu option to open the DepotDownloader folder.
- Reduce false-positive mod update results by skipping installed mods that are incompatible with the selected game version.
- Fix main content height so the launcher body padding matches the title bar layout more cleanly.
- Group related run modes in the launch dropdown with brighter separators for better readability.

## 1.5.1 (2026-03-17)
- Open the Steam login dialog immediately when a download needs authentication, instead of leaving the download retry modal behind it.
- Hide the DepotDownloader command prompt window on Windows.

## 1.5.0 (2026-03-14)
- Fix Freeze Bug

## 1.4.10 (2026-03-14)
- Change Discord Rich Presence Design
- Fix Select Menu auto select bug

## 1.4.9 (2026-03-14)
- Change Discord Rich Presence Design

## 1.4.8 (2026-03-13)
- Remember the last selected version and run mode when reopening the launcher
- Allow opening the versions folder even before any version is installed
- Improve Steam login UX
- Add Discord Rich Presence support
- Update Discord Rich Presence
- Prevent launching multiple launcher instances at the same time

## 1.4.7 (2026-03-07)
- Practice mod bug fix

## 1.4.6 (2026-03-07)
- Wesley's
  - config fix
  - Beta -> Stable

## 1.4.5 (2026-03-07)
- Support MODDED HQ
  - Brutal
  - Wesley's ʙᴇᴛᴀ
- fix issue with practice mode for linux
- change some UX
  - download UI

## 1.3.7 (2026-01-26)
- Add some v40 practice mods
  - DanceTools
  - FreeCammer

## 1.3.6 (2026-01-26)
- Change Logo

## 1.3.5 (2026-01-25)
- Fix python using user path instead of system path
- Change logo

## 1.3.4 (2026-01-24)
- Add link/unlink config folder

## 1.3.2 (2026-01-24)
- Add practice mode

## 1.3.1 (2026-01-24)
- Add practice mode

## 1.2.2 (2026-01-23)
- Fix to automatically install and run Proton on Linux (thanks Maku!)

## 1.2.1 (2026-01-22)
- Fixed a bug where some items were not applied when the remote manifest was modified.

## 1.2.0 (2026-01-22)
- Closing and reopening the download window now clears download errors/status.
- Added a Retry button to quickly re-attempt failed downloads.
- Added a Cancel download button during download; canceling deletes the in-progress version folder.
- The position of the plugin enable/disable button has been changed.
- Installed plugin versions are now displayed in the config editor.
- Fixed an issue where linked plugins were not enabled/disabled together.
- When a linked plugin exists, only one of them is now displayed instead of both.

## 1.1.2 (2025-01-21)
- Fixed an issue where you could not log in when using Steam app authentication.

## 1.1.1 (2026-01-21)
- Fixed timeout false detection issue

## 1.1.0 (2026-01-21)
- Resolving plugin installation issues caused by false antivirus detection

## 1.0.8 (2026-01-21)
- Fixed a bug in the check logic related to mode updates and omissions.
- Fixed a scrolling bug in the UI.

## 1.0.7 (2026-01-20)
- HOTFIX: Bug where submission does not work after entering 2FA
- Fixed the issue where progress was not applied when downloading a game
- Fixed 2FA code not showing when input is not required

## 1.0.6 (2026-01-20)
- Fixed the issue where progress was not applied when downloading a game
- Fixed 2FA code not showing when input is not required

## 1.0.5 (2026-01-20)
- Fix Macos build issue

## 1.0.4 (2026-01-20)
- Custom Toolbar(open version folder)

## 1.0.3 (2026-01-18)
- Initial release
