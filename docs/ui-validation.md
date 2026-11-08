# Desktop validation — 2026-11-08

The native preview was run on Linux under Hyprland/Wayland using the host Intel Iris Xe GPU with the Mesa Vulkan driver. Xvfb was unsuitable for this GPU because it did not expose DRI3 presentation; the final captures came from the actual Wayland session. Only the application's window was captured.

The native mouse workflow covered selecting books, opening the download queue, pausing, resuming, navigating to settings, switching to Simplified Chinese, returning to the queue, and signing out while paused. The script closes its demo process and restores the prior focus and pointer. All screenshots contain synthetic books and a synthetic account.

GPUI interaction tests assert login input, page navigation, selection, manual proxy edits, language changes, pause/resume state, logout cancellation, rejection of stale background events, and bounds at 960 × 640. HTTP tests use real local sockets to verify that paused workers stop writing, completed downloads match the original bytes, saved partial files survive cancellation, and interrupted connections reconnect at the saved range offset. The final-byte resume case accepts a server's `416` response only when `Content-Range` confirms that the entire partial file was already received.

Theme coverage checks system, dark, and light preferences across vibrant and non-vibrant GPUI appearance values, theme persistence, automatic system updates, manual overrides, UI switching in English and Simplified Chinese, and readable text contrast in both palettes. A native Hyprland preview captures explicit dark mode in the Library, Downloads, and Settings views.

The native `--check-system-theme` run also verified the rendered background while switching the OS setting from light to dark, then confirmed that an explicit Light preference remained light through further OS changes. The script restored the original `prefer-dark` setting afterward. Local regression results: 29 passed, with the live service test left opt-in for this appearance-only change. Clippy, the Linux release build, and the Windows test-target compile check passed.

The live account test used the configured `.env` credentials, logged in, loaded purchased books, downloaded one via the legacy endpoint using the fetched title and author for its name, decrypted it, read every ZIP entry, and checked that the LCP license and encryption files were absent. Downloaded books were held in a temporary directory and removed afterward. The native UI workflow uses demo data; it is independent of the real service test.

Linux release and Windows test-target compilation were checked locally. The repository's GitHub Actions matrix remains responsible for native Windows and macOS builds; neither platform was run interactively in this environment.

CI portability follow-up (2026-11-09): the pause/resume test now measures readable file bytes instead of directory-entry metadata, which can remain stale on Windows while a writer is open. It still asserts a partial download, no additional bytes during the pause, and byte-for-byte equality after resume. `cargo nextest run --offline --locked` passed all 29 regular tests; the targeted pause/resume stress run passed 30/30 iterations. Windows target compilation, Clippy, actionlint, and shell syntax checks passed locally. Windows native execution still needs its next CI run.

The macOS job uses the ARM64 `macos-15` image. Packaging emits an ad-hoc-signed `.app`, a ZIP preserving the app's permissions, and a DMG containing the app plus an Applications shortcut. Local packaging orchestration was checked with Apple CLI commands mocked, including bundle metadata, versioning, permissions, archive layout, and paths containing spaces. The workflow runs actual `lipo`, `plutil`, `codesign`, `unzip`, and `hdiutil` checks on macOS. Native signing/DMG creation has not been executed on the local Linux host. Checkout and artifact upload use their Node 24 v6 actions.

The workflow and packaging script now both pass the input file before the architecture command: `lipo INPUT -verify_arch arm64`. Putting the filename after `-verify_arch arm64` makes lipo interpret the filename as another architecture. Local CLI validation used Clang-generated ARM64 and x86_64 Mach-O files and the actual LLVM lipo executable: the previous argument order failed, both corrected invocations accepted ARM64 and rejected x86_64, and the packaging invocation handled a path containing spaces. Actionlint and shell syntax checks passed. Apple's Xcode lipo and native packaging still run in macOS CI.

The keyboard interaction tests use GPUI's `secondary-a` shortcut, which maps to Command+A on macOS and Control+A on Windows/Linux. A hard-coded Control+A did not select the worker field on macOS, so typing `6` appended to the existing `4` and produced `46`, which the settings limit reduced to `16`. Both select-all uses now follow the platform shortcut. Assertions check that selection occurred, that replacement leaves exactly `6`, and that select-all/backspace actually clears the library search before sign out. Local Linux `cargo test` and Nextest each passed all 39 regular tests, with the live service test excluded; Clippy passed with `-D warnings`. Native macOS execution remains for CI.

Queue and export controls follow-up (2026-11-09): each pending or downloading book now has Pause/Resume. Pausing a transfer retains its partial files and lets the next unpaused book run; resume waits for the current transfer and reuses the saved offsets. Toolbar controls pause/resume all and clear completed records. The clear action retains actual files and the purchased library. Exporting tasks finish their export before the scheduler moves on.

Single-thread downloads are now the default, including when loading older settings with a saved worker count. Parallel transfers require explicitly enabling experimental mode in Settings, with 2–16 connections per book. Encrypted originals are removed after a successful export unless Keep encrypted originals is enabled. Failed exports retain the source and any previous export; temporary exports are cleaned up. Cleanup failures retain a completed export and show a warning. These preferences are saved and apply to the next queue.

`cargo nextest run --offline --locked` passed 39 regular tests with the live service test skipped. The new real HTTP queue test passed 20/20 stress iterations: it pauses the first book, downloads the second, verifies no change to the first partial file, and resumes the first to byte-for-byte equality. GPUI mouse tests cover individual and global controls, clearing while active and after the last completion, settings, language changes, and compact bounds. Filesystem tests cover default cleanup, explicit source retention, failed export preservation, staging cleanup, and cleanup warnings. The existing synthetic LCP fixture now also checks actual export through the cleanup wrapper.

The opt-in live test passed using the configured `.env` account: login, purchased-book listing, legacy download with one connection, sanitized EPUB export, and automatic encrypted-source cleanup. Archive entries were read back and the absent license/encryption entries were checked. All downloaded and exported files were in an automatically removed temporary directory. No account values or traffic captures were used in screenshots.

The native Hyprland smoke script ran with the release binary and synthetic data. Captures were inspected for clear-completed, individual pause/resume, the active next book, global pause/resume, experimental parallel mode and its worker field, returning to single-thread mode, source-retention toggle, English/Chinese, dark/light, sign out, and the 960 × 640 layout. The script restored focus and pointer position. Linux release build, Clippy with `-D warnings`, Windows GNU test-target compilation, and actionlint passed. Native Windows/macOS execution remains for CI.

- [Completed queue and per-task controls](screenshots/downloads-controls.png)
- [Paused task with the next book downloading](screenshots/download-task-paused.png)
- [Compact downloads](screenshots/downloads-compact.png)
- [Chinese download and export settings](screenshots/settings-downloads-zh.png)

- [Library](screenshots/library.png)
- [Paused downloads in Chinese](screenshots/paused-zh.png)
- [Settings in Chinese](screenshots/settings-zh.png)
- [Signed out](screenshots/signed-out.png)
- [Dark library](screenshots/library-dark.png)
- [Dark downloads](screenshots/downloads-dark.png)
- [Dark settings](screenshots/settings-dark.png)
