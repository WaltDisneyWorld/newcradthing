# iCard

A desktop tool that customizes the artwork of **Apple Wallet cards** and the
**passcode keypad** on an iPhone you own, connected over USB or paired WiFi.

This is a from-scratch recreation of the Windows "iCard" utility (a Rust +
[`egui`](https://github.com/emilk/egui) app). It is in the same family as the
open-source iOS-customization tools [Cowabunga](https://github.com/leminlimez/cowabunga)
and [Nugget](https://github.com/leminlimez/Nugget): it talks to a *trusted*,
cabled device and swaps cosmetic assets. No accounts, no analytics, no network
calls.

## What it does

- **Scan a card hash.** On the Wallet tab, click *Scan*, open Apple Wallet on
  the iPhone and tap a pass. iCard reads the device syslog and captures that
  pass's on-disk hash automatically. Captured hashes are saved for next time.
- **Apply a card skin.** Pick any PNG/JPG/WebP, drag inside the preview to
  reposition the crop, and *Write to iPhone*. iCard renders the Wallet asset
  trio (`cardBackgroundCombined@3x.png`, `@2x.png` and `cardBackgroundCombined.pdf`)
  and writes it to the pass. It backs up the original first so you can
  *Restore Original*.
- **Passcode keypad themes.** Load a `.passthm` package (Cowabunga/Nugget
  format) and flash custom dialer-button artwork, optionally generating the
  `-bold` variants for devices with Bold Text enabled.
- **Logs tab.** Everything is logged; copy to clipboard or save
  `icard-diagnostics.log`.
- **English / Español**, switchable at runtime.

## Where it stands

| Part | State |
|---|---|
| UI (Wallet / Passcode / Logs, transport bar, setup guide, activation) | Complete. Runs today. |
| Image pipeline (crop → 1536×969, @3x/@2x PNG + PDF) | Complete and unit-exercised. |
| Device enumeration (usbmux `ListDevices`) | Implemented in pure Rust; works against a running Apple Mobile Device Service / usbmuxd. |
| Lockdown / AFC / syslog-relay clients | Implemented to the documented protocols. |
| Privileged write into `Passes/Cards` (streaming-zip conduit) | **Scaffolded, not a turnkey exploit — see "Honest scope".** |
| Apple Mobile Device Support (TLS service sessions) | Detected on Windows; the secured-session hand-off is stubbed. |

It compiles and runs on Linux/macOS/Windows. The full device write-path is only
meaningfully exercisable on **Windows with Apple Mobile Device Support
installed** and a trusted iPhone attached — which is the environment the
original targeted.

## Honest scope

Wallet card artwork lives at `/var/mobile/Library/Passes/Cards/<hash>/`, which is
**outside** the `com.apple.afc` media sandbox. Cowabunga/Nugget-class tools reach
it on a trusted, cabled device via the Books/AirTraffic
`com.apple.streaming_zip_conduit` path, using a staging archive whose
`com.apple.ZipMetadata.plist` relative paths resolve into that tree.

This recreation implements every **legitimate** transport layer for real
(usbmux, lockdown, AFC, syslog relay, the image/PDF pipeline, the saved-card
database). The privileged conduit is assembled with the correct service names,
archive layout and manifest structure (`src/wallet/airlift.rs`), but it is **not**
delivered as a verified, tested sandbox escape — the TLS service session it needs
is handed to Apple Mobile Device Support rather than hand-forged here, and there
is no device in this build to validate against. Use it only on hardware you own.

## Build

Needs a recent stable Rust toolchain.

```bash
cargo run --release
```

On Windows, install 64-bit iTunes or the standalone **Apple Mobile Device
Support** package first, connect the iPhone by USB and tap *Trust This Computer*
once.

## Activation

iCard gates its device-writing actions behind a **local** activation code
(checked entirely offline; state in `%LOCALAPPDATA%\AirCard\activation.json`).
The code is 16 base-32 symbols with a trailing checksum. Valid demo codes:

```
ABCD-EFGH-JKLM-NPQ3
NPQR-STUV-WXYZ-234P
```

## Layout

| Path | What it is |
|---|---|
| `src/main.rs` | Entry point / window setup. |
| `src/app.rs`, `src/app/ui_sections.rs` | The egui app: state machine and views. |
| `src/i18n.rs` | English / Spanish strings. |
| `src/model.rs` | Shared types. |
| `src/activation.rs` | Local activation gate. |
| `src/saved_cards.rs` | Settings + saved-card database. |
| `src/passthm.rs` | `.passthm` passcode-theme loader + flasher. |
| `src/device/` | usbmux, lockdown, AFC, syslog, Apple-support detection. |
| `src/wallet/` | Image pipeline, card airlift, hash scanner. |

## Files on disk

Everything lives under `%LOCALAPPDATA%\AirCard` (Windows) or
`~/.local/share/AirCard` (Linux/macOS): `settings.json`, `cards.json`,
`activation.json`, and `originals/<hash>/` for restore backups.

## Legal / respect

This controls cosmetic assets on **your own** device. The Apple Wallet, iTunes
and iOS names belong to Apple. Don't use it to impersonate payment cards,
identity documents or anyone else's branding.
