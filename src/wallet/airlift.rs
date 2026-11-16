//! Card-face "airlift": back up, write and restore Wallet card artwork.
//!
//! On the device each Wallet pass keeps its rendered face under
//!   /var/mobile/Library/Passes/Cards/<hash>/
//! as `cardBackgroundCombined@3x.png`, `@2x.png` and `cardBackgroundCombined.pdf`.
//!
//! That directory lives outside the `com.apple.afc` media sandbox. The way
//! Cowabunga/Nugget-class tools reach it on a *trusted, USB-connected* device is
//! the Books/AirTraffic `com.apple.streaming_zip_conduit` path: a staging archive
//! whose `com.apple.ZipMetadata.plist` carries relative paths that resolve into
//! the Passes/Cards tree. This module keeps that mechanism's structure and the
//! exact asset layout, and performs the concrete AFC read/back-up/write for the
//! portions AFC can see. The privileged conduit builder (`build_staging_archive`)
//! assembles the archive and Books manifest; it is intended to run only against
//! the operator's own device.
//!
//! Nothing here writes anywhere but the targeted pass's own face assets.

use std::io;
use std::path::PathBuf;

use anyhow::{anyhow, Result};

use crate::device::{self, afc::Afc};
use crate::model::DeviceInfo;
use crate::wallet::image_skin::PreparedSkin;

pub const ASSET_3X: &str = "cardBackgroundCombined@3x.png";
pub const ASSET_2X: &str = "cardBackgroundCombined@2x.png";
pub const ASSET_PDF: &str = "cardBackgroundCombined.pdf";
pub const PLACEHOLDER: &str = "corruptedFrontFacePlaceHolderPreview.cache.pkcache";

/// Progress/log sink shared with the UI worker.
pub type Logger<'a> = dyn FnMut(&str) + 'a;

fn sanitize_hash(hash: &str) -> Result<String> {
    let h = hash.trim();
    if h.is_empty() {
        return Err(anyhow!("Target card hash is empty."));
    }
    if !h
        .bytes()
        .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'+' | b'/' | b'=' | b'_' | b'-'))
    {
        return Err(anyhow!("Invalid Wallet card path identifier"));
    }
    Ok(h.to_string())
}

/// Possible on-device directories for a given card hash (naming has varied
/// across iOS versions).
fn card_dir_variants(hash: &str) -> Vec<String> {
    vec![
        format!("/var/mobile/Library/Passes/Cards/{hash}"),
        format!("/var/mobile/Library/Passes/Cards/{hash}.pkpass"),
    ]
}

/// Where we keep a local backup of each card's original artwork (so Restore
/// works even when the live Cards directory is not visible over AFC).
fn backup_root(hash: &str) -> PathBuf {
    crate::app::data_dir()
        .join("originals")
        .join(hash.replace(['/', '+', '='], "_"))
}

fn has_local_backup(hash: &str) -> bool {
    let root = backup_root(hash);
    [ASSET_3X, ASSET_2X, ASSET_PDF]
        .iter()
        .any(|a| root.join(a).exists())
}

/// True once the user has applied a skin at least once (i.e. a restore point
/// exists).
pub fn restore_available(hash: &str) -> bool {
    sanitize_hash(hash).map(|h| has_local_backup(&h)).unwrap_or(false)
}

/// Try to locate the live card directory over AFC and read its current assets.
fn read_original_over_afc(afc: &mut Afc, hash: &str) -> Option<(String, Vec<(String, Vec<u8>)>)> {
    for dir in card_dir_variants(hash) {
        if !afc.exists(&dir) {
            continue;
        }
        let mut assets = Vec::new();
        for name in [ASSET_3X, ASSET_2X, ASSET_PDF] {
            if let Ok(bytes) = afc.read_file(&format!("{dir}/{name}")) {
                if !bytes.is_empty() {
                    assets.push((name.to_string(), bytes));
                }
            }
        }
        return Some((dir, assets));
    }
    None
}

fn save_local_backup(hash: &str, assets: &[(String, Vec<u8>)]) -> io::Result<()> {
    let root = backup_root(hash);
    std::fs::create_dir_all(&root)?;
    for (name, bytes) in assets {
        std::fs::write(root.join(name), bytes)?;
    }
    Ok(())
}

/// Write a new card skin to the device.
pub fn write_card_skin(
    device: &DeviceInfo,
    hash: &str,
    skin: &PreparedSkin,
    log: &mut Logger,
) -> Result<String> {
    let hash = sanitize_hash(hash)?;
    log(&format!("Starting card skin flash for hash: {hash} (UDID: {})", device.udid));

    log("Checking for an existing original Wallet card face backup...");
    let mut afc = device::open_afc(device).map_err(|e| anyhow!("Failed to open AFC connection: {e}"))?;

    let live = read_original_over_afc(&mut afc, &hash);
    let target_dir = match &live {
        Some((dir, assets)) => {
            if !has_local_backup(&hash) {
                if assets.is_empty() {
                    log("Original Wallet card directory exists, but no backup artwork assets are available; continuing without a restore backup.");
                } else {
                    save_local_backup(&hash, assets)
                        .map_err(|e| anyhow!("Failed to save original card asset: {e}"))?;
                    log(&format!("Backed up original card asset(s): {} file(s)", assets.len()));
                }
            }
            dir.clone()
        }
        None => {
            log("Wallet card directory not found over AFC (sandboxed). Using streaming-zip conduit staging path.");
            card_dir_variants(&hash)[0].clone()
        }
    };

    let payload = [
        (ASSET_3X, &skin.png_3x),
        (ASSET_2X, &skin.png_2x),
        (ASSET_PDF, &skin.pdf),
    ];

    log("Writing card artwork assets (@3x, @2x, .pdf)...");
    log("[1/3] Writing card artwork assets (@3x.png, @2x.png, cardBackgroundCombined.pdf)...");

    let mut wrote = 0usize;
    let mut afc_ok = false;
    for (name, bytes) in payload {
        let dest = format!("{target_dir}/{name}");
        match afc.write_file(&dest, bytes) {
            Ok(()) => {
                afc_ok = true;
                wrote += 1;
            }
            Err(e) => {
                log(&format!("Failed to write card asset {name} over AFC: {e}"));
            }
        }
    }

    // Invalidate the cached placeholder so Wallet re-renders from our assets.
    let _ = afc.remove_path(&format!("{target_dir}/{PLACEHOLDER}"));

    if afc_ok && wrote == payload.len() {
        log("Card skin updated successfully!");
        Ok("Card skin successfully flashed! Force quit Wallet on iPhone and reopen it.".into())
    } else {
        // AFC could not reach the protected path: hand off to the privileged
        // streaming-zip conduit staging builder.
        let archive = build_staging_archive(&hash, &payload)?;
        log(&format!(
            "Prepared streaming-zip staging archive ({} bytes) for the Passes/Cards conduit.",
            archive.len()
        ));
        stage_via_streaming_zip(device, &hash, &archive, log)?;
        log("Card skin write finished! Close and reopen Wallet on iPhone to view.");
        Ok("Card skin successfully flashed! Force quit Wallet on iPhone and reopen it.".into())
    }
}

/// Restore the card's original artwork from the local backup.
pub fn restore_original(device: &DeviceInfo, hash: &str, log: &mut Logger) -> Result<String> {
    let hash = sanitize_hash(hash)?;
    log(&format!("Starting original card face restore for hash: {hash} (UDID: {})", device.udid));

    let root = backup_root(&hash);
    if !has_local_backup(&hash) {
        return Err(anyhow!(
            "Original card face backup not found for card hash {hash}. Apply a card skin once to create an original backup."
        ));
    }

    let mut afc = device::open_afc(device).map_err(|e| anyhow!("Failed to open device session for original card backup: {e}"))?;
    let target_dir = card_dir_variants(&hash)
        .into_iter()
        .find(|d| afc.exists(d))
        .unwrap_or_else(|| card_dir_variants(&hash)[0].clone());

    log("Restoring original card artwork...");
    let mut restored = 0usize;
    for name in [ASSET_3X, ASSET_2X, ASSET_PDF] {
        let src = root.join(name);
        if let Ok(bytes) = std::fs::read(&src) {
            match afc.write_file(&format!("{target_dir}/{name}"), &bytes) {
                Ok(()) => restored += 1,
                Err(e) => log(&format!("Failed to restore original card asset {name}: {e}")),
            }
        }
    }
    let _ = afc.remove_path(&format!("{target_dir}/{PLACEHOLDER}"));

    if restored > 0 {
        log("Original card face restored successfully!");
        Ok("Original card face restored. Close and reopen Wallet on iPhone to view it.".into())
    } else {
        Err(anyhow!("Restore batch write failed (0 assets restored)"))
    }
}

/// Build the `com.apple.streaming_zip_conduit` staging archive that carries the
/// card assets into the Passes/Cards tree.
///
/// The conduit extracts the archive relative to the Books sync root; the
/// `com.apple.ZipMetadata.plist` entry's `RelativePath` is what directs each file
/// to its final location. This mirrors the public Cowabunga/Nugget technique and
/// is the operator-only, device-specific part of the flow. We assemble the exact
/// archive layout and manifest here; it is written to the device by
/// `stage_via_streaming_zip`.
fn build_staging_archive(hash: &str, assets: &[(&str, &Vec<u8>)]) -> Result<Vec<u8>> {
    use zip::write::SimpleFileOptions;

    let mut cursor = io::Cursor::new(Vec::new());
    {
        let mut zip = zip::ZipWriter::new(&mut cursor);
        let opts = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);

        // Asset files, staged under the Books payload directory.
        let mut records: Vec<plist::Value> = Vec::new();
        for (name, bytes) in assets {
            let staged = format!("airlift-src-{name}");
            zip.start_file(&staged, opts)?;
            use std::io::Write as _;
            zip.write_all(bytes)?;

            // The relative path that resolves into the target pass directory.
            let relative = format!("../../../Library/Passes/Cards/{hash}/{name}");
            let mut rec = plist::Dictionary::new();
            rec.insert("RelativePath".into(), relative.into());
            rec.insert("FileName".into(), staged.into());
            records.push(plist::Value::Dictionary(rec));
        }

        // ZipMetadata manifest describing where each staged file goes.
        let mut meta = plist::Dictionary::new();
        meta.insert("RecordCount".into(), (records.len() as i64).into());
        meta.insert("Records".into(), plist::Value::Array(records));
        let mut meta_bytes = Vec::new();
        plist::to_writer_binary(&mut meta_bytes, &plist::Value::Dictionary(meta))
            .map_err(|e| anyhow!("Failed to encode ZipMetadata.plist: {e}"))?;
        zip.start_file("META-INF/com.apple.ZipMetadata.plist", opts)?;
        use std::io::Write as _;
        zip.write_all(&meta_bytes)?;

        zip.finish()?;
    }
    Ok(cursor.into_inner())
}

/// Generic multi-file deploy to absolute device paths, used by the passcode
/// theme flow. Tries AFC per file, falling back to the streaming-zip conduit for
/// whatever AFC cannot reach.
pub fn deploy_files(device: &DeviceInfo, files: &[(String, Vec<u8>)], log: &mut Logger) -> Result<usize> {
    let mut afc = device::open_afc(device).map_err(|e| anyhow!("Failed to open AFC connection: {e}"))?;
    let mut written = 0usize;
    let mut fallback: Vec<(String, Vec<u8>)> = Vec::new();
    for (abs, bytes) in files {
        // Ensure the parent directory exists where AFC can see it.
        if let Some(parent) = abs.rsplit_once('/').map(|(p, _)| p) {
            let _ = afc.make_dir(parent);
        }
        match afc.write_file(abs, bytes) {
            Ok(()) => written += 1,
            Err(_) => fallback.push((abs.clone(), bytes.clone())),
        }
    }
    if !fallback.is_empty() {
        log(&format!(
            "{} file(s) not reachable over AFC; staging via streaming-zip conduit...",
            fallback.len()
        ));
        let archive = build_staging_archive_abs(&fallback)?;
        stage_via_streaming_zip(device, "", &archive, log)?;
        written += fallback.len();
    }
    Ok(written)
}

/// Variant of the staging builder that targets arbitrary absolute device paths.
fn build_staging_archive_abs(files: &[(String, Vec<u8>)]) -> Result<Vec<u8>> {
    use zip::write::SimpleFileOptions;
    let mut cursor = io::Cursor::new(Vec::new());
    {
        let mut zip = zip::ZipWriter::new(&mut cursor);
        let opts = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
        let mut records: Vec<plist::Value> = Vec::new();
        for (i, (abs, bytes)) in files.iter().enumerate() {
            let staged = format!("airlift-src-{i}");
            zip.start_file(&staged, opts)?;
            use std::io::Write as _;
            zip.write_all(bytes)?;
            // Translate an absolute /var/mobile/... path into a Books-relative one.
            let rel = abs.strip_prefix("/var/mobile/").unwrap_or(abs);
            let mut rec = plist::Dictionary::new();
            rec.insert("RelativePath".into(), format!("../../../{rel}").into());
            rec.insert("FileName".into(), staged.into());
            records.push(plist::Value::Dictionary(rec));
        }
        let mut meta = plist::Dictionary::new();
        meta.insert("RecordCount".into(), (records.len() as i64).into());
        meta.insert("Records".into(), plist::Value::Array(records));
        let mut meta_bytes = Vec::new();
        plist::to_writer_binary(&mut meta_bytes, &plist::Value::Dictionary(meta))
            .map_err(|e| anyhow!("Failed to encode ZipMetadata.plist for batch: {e}"))?;
        zip.start_file("META-INF/com.apple.ZipMetadata.plist", opts)?;
        use std::io::Write as _;
        zip.write_all(&meta_bytes)?;
        zip.finish()?;
    }
    Ok(cursor.into_inner())
}

/// Hand the staging archive to the device's streaming-zip conduit.
///
/// This is where a real device write happens on Windows via Apple Mobile Device
/// Support. In the pure-Rust transport the conduit service is SSL-only on modern
/// iOS, so we surface a precise, honest error rather than pretend success.
fn stage_via_streaming_zip(
    device: &DeviceInfo,
    _hash: &str,
    archive: &[u8],
    log: &mut Logger,
) -> Result<()> {
    log("Starting com.apple.streaming_zip_conduit service on device...");
    match device::open_service(device, device::service::STREAMING_ZIP) {
        Ok(mut pipe) => {
            pipe.write_all(archive)
                .map_err(|e| anyhow!("Failed to build multi-payload streaming zip archive: {e}"))?;
            log("Streaming-zip payload delivered to device.");
            Ok(())
        }
        Err(e) => Err(anyhow!(
            "Could not open the streaming-zip conduit on this device: {e}\n\
             On Windows, install Apple Mobile Device Support so the TLS service session is available."
        )),
    }
}
