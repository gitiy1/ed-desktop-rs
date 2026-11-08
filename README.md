# Endao Desktop

Native GPUI client for Windows, macOS, and Linux. The project is intentionally self-contained and uses a Rust implementation for login, purchased-book listing, legacy book downloads, resumable HTTP ranges, and sanitized LCP EPUB export.

## Build

Install Rust stable and the native desktop build dependencies for your platform, then run:

```sh
cargo build --release --locked
```

Linux needs the X11/Wayland, XKB, fontconfig, and OpenGL development packages. The GitHub Actions workflow builds release binaries on Ubuntu, macOS, and Windows.

## Use

Start the app with `cargo run --release --locked`, enter the account email and password, select a download folder, choose one or more purchased books, and start the queue. The client defaults to the legacy `POST /api/v4/book/download` endpoint. It probes for byte-range support using the first download response; when supported it downloads parts in parallel and retains them for restart, otherwise it streams to a resumable `.part` file.

The interface follows the system language by default and supports English and Simplified Chinese. Proxy mode defaults to system settings, with manual HTTP and SOCKS5 options. System mode reads static proxy settings supported by the OS integration and falls back to `HTTP_PROXY`/`HTTPS_PROXY` environment variables. PAC files and authenticated proxy URLs are not supported; enter proxy endpoints as `host:port`.

Run `cargo test --locked` for the GPUI interaction tests and network/DRM unit tests. The UI test drives real GPUI mouse and keyboard events at the default 1180 x 820 window size and checks that the proxy field remains usable.

Decrypted exports are written as ordinary `.epub` files. The encrypted source is kept alongside it as `.books.epub`. LCP license and encryption files, plus account-related OPF metadata, are removed from the exported copy. The password is kept in memory only; the app saves the output directory, worker count, language, and proxy settings in the platform config directory.

The default client identity uses generic values (`Android Device`, `Android SDK_35`) and a newly generated device ID. It does not use device identifiers copied from packet captures.
