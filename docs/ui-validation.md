# Desktop validation — 2026-11-08

The native preview was run on Linux under Hyprland/Wayland using the host Intel Iris Xe GPU with the Mesa Vulkan driver. Xvfb was unsuitable for this GPU because it did not expose DRI3 presentation; the final captures came from the actual Wayland session. Only the application's window was captured.

The native mouse workflow covered selecting books, opening the download queue, pausing, resuming, navigating to settings, switching to Simplified Chinese, returning to the queue, and signing out while paused. The script closes its demo process and restores the prior focus and pointer. All screenshots contain synthetic books and a synthetic account.

GPUI interaction tests assert login input, page navigation, selection, manual proxy edits, language changes, pause/resume state, logout cancellation, rejection of stale background events, and bounds at 960 × 640. HTTP tests use real local sockets to verify that paused workers stop writing, completed downloads match the original bytes, saved partial files survive cancellation, and interrupted connections reconnect at the saved range offset. The final-byte resume case accepts a server's `416` response only when `Content-Range` confirms that the entire partial file was already received.

Theme coverage checks system, dark, and light preferences across vibrant and non-vibrant GPUI appearance values, theme persistence, automatic system updates, manual overrides, UI switching in English and Simplified Chinese, and readable text contrast in both palettes. A native Hyprland preview captures explicit dark mode in the Library, Downloads, and Settings views.

The native `--check-system-theme` run also verified the rendered background while switching the OS setting from light to dark, then confirmed that an explicit Light preference remained light through further OS changes. The script restored the original `prefer-dark` setting afterward. Local regression results: 29 passed, with the live service test left opt-in for this appearance-only change. Clippy, the Linux release build, and the Windows test-target compile check passed.

The live account test used the configured `.env` credentials, logged in, loaded purchased books, downloaded one via the legacy endpoint using the fetched title and author for its name, decrypted it, read every ZIP entry, and checked that the LCP license and encryption files were absent. Downloaded books were held in a temporary directory and removed afterward. The native UI workflow uses demo data; it is independent of the real service test.

Linux release and Windows test-target compilation were checked locally. The repository's GitHub Actions matrix remains responsible for native Windows and macOS builds; neither platform was run interactively in this environment.

- [Library](screenshots/library.png)
- [Paused downloads in Chinese](screenshots/paused-zh.png)
- [Settings in Chinese](screenshots/settings-zh.png)
- [Signed out](screenshots/signed-out.png)
- [Dark library](screenshots/library-dark.png)
- [Dark downloads](screenshots/downloads-dark.png)
- [Dark settings](screenshots/settings-dark.png)
