//! `.passthm` passcode-keypad theme packages (Cowabunga / Nugget format).
//!
//! A `.passthm` is a zip of dialer-button artwork. Each entry names a key
//! (0–9, * or #) and optionally a variant. iCard unpacks it, maps the buttons to
//! the device's TelephonyUI bundle filenames and deploys them.

use std::io::Read;
use std::path::Path;

use anyhow::{anyhow, Context, Result};
use regex::Regex;

use crate::model::{DeviceInfo, KeypadLanguage};
use crate::wallet::airlift;

#[derive(Clone)]
pub struct DialerButton {
    pub key: char,
    pub variant: Option<String>,
    pub png: Vec<u8>,
}

#[derive(Clone)]
pub struct PasscodeTheme {
    pub name: String,
    pub buttons: Vec<DialerButton>,
}

impl PasscodeTheme {
    pub fn button_count(&self) -> usize {
        self.buttons.len()
    }
}

/// Load and classify a `.passthm` archive.
pub fn load(path: &Path) -> Result<PasscodeTheme> {
    let file = std::fs::File::open(path)
        .with_context(|| "Failed to open passcode theme file")?;
    let mut zip = zip::ZipArchive::new(file)
        .with_context(|| "Failed to read theme file as zip archive")?;

    // Entry names look like `telephony-1.png`, `1-bold.png`, `custom-#-x.png`.
    let re = Regex::new(r"(?:^[a-zA-Z]+-)?([0-9*#])(?:-([^-\n.]+))?").unwrap();

    let mut buttons = Vec::new();
    for i in 0..zip.len() {
        let mut entry = zip.by_index(i)?;
        if entry.is_dir() {
            continue;
        }
        let name = entry.name().rsplit('/').next().unwrap_or("").to_string();
        if !name.to_ascii_lowercase().ends_with(".png") {
            continue;
        }
        let Some(caps) = re.captures(&name) else { continue };
        let key = caps.get(1).and_then(|m| m.as_str().chars().next());
        let Some(key) = key else { continue };
        let variant = caps.get(2).map(|m| m.as_str().to_string());

        let mut png = Vec::new();
        entry.read_to_end(&mut png)?;
        if png.is_empty() {
            continue;
        }
        buttons.push(DialerButton { key, variant, png });
    }

    if buttons.is_empty() {
        return Err(anyhow!("CustomTheme contained no recognisable dialer buttons"));
    }

    let name = path
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| "CustomTheme".into());
    Ok(PasscodeTheme { name, buttons })
}

/// The on-device directory that holds dialer artwork for a TelephonyUI bundle.
fn bundle_dir(lang: KeypadLanguage) -> String {
    format!(
        "/var/mobile/Library/Caches/com.apple.springboard/{}",
        lang.bundle()
    )
}

/// On-device filenames a given button maps to (`telephony-N` and `telephonyui-N`).
fn target_names(key: char, bold: bool) -> Vec<String> {
    let k = match key {
        '*' => "star".to_string(),
        '#' => "pound".to_string(),
        d => d.to_string(),
    };
    let mut names = vec![format!("telephony-{k}.png"), format!("telephonyui-{k}.png")];
    if bold {
        names.push(format!("telephony-{k}-bold.png"));
        names.push(format!("telephonyui-{k}-bold.png"));
    }
    names
}

/// Flash a passcode theme to the device.
pub fn apply(
    device: &DeviceInfo,
    theme: &PasscodeTheme,
    lang: KeypadLanguage,
    generate_bold: bool,
    log: &mut airlift::Logger,
) -> Result<String> {
    log(&format!("Flashing passcode theme '{}' ({} buttons)", theme.name, theme.button_count()));
    log("Writing passcode theme buttons...");

    let dir = bundle_dir(lang);
    let mut files: Vec<(String, Vec<u8>)> = Vec::new();
    for b in &theme.buttons {
        // A button may be an explicit bold variant already; otherwise it feeds
        // both the normal slot and (optionally) a generated bold slot.
        let is_bold_source = b.variant.as_deref() == Some("bold");
        for name in target_names(b.key, generate_bold && !is_bold_source) {
            if is_bold_source && !name.contains("-bold") {
                continue;
            }
            files.push((format!("{dir}/{name}"), b.png.clone()));
        }
    }

    let n = airlift::deploy_files(device, &files, log)?;
    log(&format!("Passcode theme applied successfully! ({n} file(s) written)"));
    Ok("Passcode theme successfully written! Lock or reboot iPhone to see new keypad.".into())
}
