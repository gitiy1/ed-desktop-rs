# Desktop validation — 2026-11-08

The native preview was run on Linux under Hyprland/Wayland using the host Intel Iris Xe GPU with the Mesa Vulkan driver. Xvfb was unsuitable for this GPU because it did not expose DRI3 presentation; the final captures came from the actual Wayland session. Only the application's window was captured.

The native mouse workflow covered selecting books, opening the download queue, pausing, resuming, navigating to settings, switching to Simplified Chinese, returning to the queue, and signing out while paused. The script closes its demo process and restores the prior focus and pointer. All screenshots contain synthetic books and a synthetic account.

GPUI interaction tests assert login input, page navigation, selection, manual proxy edits, language changes, pause/resume state, logout cancellation, rejection of stale background events, and bounds at 960 × 640. HTTP tests use real local sockets to verify that paused workers stop writing, completed downloads match the original bytes, saved partial files survive cancellation, and interrupted connections reconnect at the saved range offset. The final-byte resume case accepts a server's `416` response only when `Content-Range` confirms that the entire partial file was already received.

The live account test used the configured `.env` credentials, logged in, loaded purchased books, downloaded one via the legacy endpoint using the fetched title and author for its name, decrypted it, read every ZIP entry, and checked that the LCP license and encryption files were absent. Downloaded books were held in a temporary directory and removed afterward. The native UI workflow uses demo data; it is independent of the real service test.

Linux release and Windows test-target compilation were checked locally. The repository's GitHub Actions matrix remains responsible for native Windows and macOS builds; neither platform was run interactively in this environment.

- [Library](screenshots/library.png)
- [Paused downloads in Chinese](screenshots/paused-zh.png)
- [Settings in Chinese](screenshots/settings-zh.png)
- [Signed out](screenshots/signed-out.png)
