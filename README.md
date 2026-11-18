# ProCard

A guided desktop **studio** for customizing **Apple Wallet card artwork** and the
**passcode keypad** on an iPhone you own, connected over USB or paired WiFi.

ProCard is a Rust + [`egui`](https://github.com/emilk/egui) app. It's in the same
family as the open-source iOS-customization tools
[Cowabunga](https://github.com/leminlimez/cowabunga) and
[Nugget](https://github.com/leminlimez/Nugget): it talks to a *trusted*, cabled
device and swaps cosmetic assets. No accounts, no analytics, no network calls.

## The studio

A sidebar + guided flow, redesigned from the ground up:

- **Sidebar** — device status, transport (Auto / USB / WiFi), your saved cards,
  EN / IT language, activation and help.
- **Step ribbon** — Connect → Scan card → Choose artwork → Apply, lit up as you go.
- **Card preview** — a large 1536 × 969 canvas; **drag to reposition** and a
  **zoom** slider to frame the art.
- **Artwork browser** — searchable packs of card-face designs, plus
  **Upload your own** (PNG / JPG / WebP).
- **Passcode theme** — load a `.passthm` package (Cowabunga / Nugget format) and
  flash a custom lockscreen keypad, with optional bold variants.
- **Activity log** — copy / save `procard-diagnostics.log`.

## About the artwork packs

The bundled packs (**Aurora, Carbon, Citrus, Marble, Nocturne** — 20 designs) are
**original, procedurally generated abstract art**, created by
`examples/gen_art.rs`. They contain no brands, logos, characters, or real card
designs.

The browser simply shows whatever an `assets/artwork/catalog.json` lists, so you
can **add your own packs**: drop a folder of your own rights-cleared images under
`assets/artwork/<pack>/` and add a catalog entry. ProCard will not ship card art
that depicts third-party brands, franchises/characters, or real people — bring
your own for anything like that, and only use art you have the rights to.

Regenerate the bundled samples anytime:

```bash
cargo run --release --example gen_art
```

## Honest scope

Wallet card artwork lives at `/var/mobile/Library/Passes/Cards/<hash>/`, which is
**outside** the `com.apple.afc` media sandbox. Cowabunga/Nugget-class tools reach
it on a trusted, cabled device via the Books/AirTraffic
`com.apple.streaming_zip_conduit` path. ProCard implements every **legitimate**
transport layer for real (usbmux, lockdown, AFC, syslog relay, the image/PDF
pipeline, the saved-card database). The privileged conduit is assembled with the
correct service names, archive layout and manifest structure
(`src/wallet/airlift.rs`), but it is **not** shipped as a verified, tested
sandbox escape — the TLS service session it needs is handed to Apple Mobile
Device Support rather than hand-forged here. Use it only on hardware you own.

## Run it

Needs a recent stable Rust toolchain.

```bash
cargo run --release
```

Or grab the prebuilt Windows package: **`dist/procard.exe`** (keep the
`assets/artwork` folder beside it so the packs load; the CI workflow builds a
ready `ProCard-win64` bundle with both). On Windows, install 64-bit iTunes or the
standalone **Apple Mobile Device Support** first, connect the iPhone by USB and
tap *Trust This Computer* once.

## Activation

Device-writing actions are gated behind a **local** activation code (checked
entirely offline; state in `%LOCALAPPDATA%\ProCard\activation.json`). The code is
16 base-32 symbols with a trailing checksum. Valid demo codes:

```
ABCD-EFGH-JKLM-NPQ3
NPQR-STUV-WXYZ-234P
```

## Layout

| Path | What it is |
|---|---|
| `src/main.rs` | Entry point / window setup. |
| `src/app.rs` | App state, worker-thread orchestration, image pipeline glue. |
| `src/app/studio.rs` | The redesigned studio UI (sidebar, steps, browser, preview). |
| `src/catalog.rs` | Artwork catalog loader. |
| `src/i18n.rs` | UI strings (EN base; IT in the studio). |
| `src/model.rs` | Shared types. |
| `src/activation.rs` | Local activation gate. |
| `src/saved_cards.rs` | Settings + saved-card database. |
| `src/passthm.rs` | `.passthm` passcode-theme loader + flasher. |
| `src/device/` | usbmux, lockdown, AFC, syslog, Apple-support detection. |
| `src/wallet/` | Image pipeline, card airlift, hash scanner. |
| `examples/gen_art.rs` | Generates the original sample packs + `catalog.json`. |
| `assets/artwork/` | Bundled original packs and their index. |

## Files on disk

Under `%LOCALAPPDATA%\ProCard` (Windows) or `~/.local/share/ProCard`
(Linux/macOS): `settings.json`, `cards.json`, `activation.json`, and
`originals/<hash>/` for restore backups.

## Legal / respect

ProCard controls cosmetic assets on **your own** device. Apple Wallet, iTunes and
iOS are Apple's. Don't use it to impersonate payment cards, identity documents,
or anyone else's branding.
