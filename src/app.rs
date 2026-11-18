//! The iCard application: egui UI, state and worker-thread orchestration.

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, Sender};
use std::sync::Arc;

use egui::Vec2;
use image::DynamicImage;

use crate::device::{self, apple::SupportStatus};
use crate::i18n::Strings;
use crate::model::{DeviceInfo, SavedCard, Settings};
use crate::wallet::{airlift, image_skin, scanner};
use crate::{activation::Activation, passthm, saved_cards};

/// Per-user data directory (`%LOCALAPPDATA%\ProCard` on Windows).
pub fn data_dir() -> PathBuf {
    if let Ok(local) = std::env::var("LOCALAPPDATA") {
        return PathBuf::from(local).join("ProCard");
    }
    if let Ok(xdg) = std::env::var("XDG_DATA_HOME") {
        return PathBuf::from(xdg).join("ProCard");
    }
    if let Ok(home) = std::env::var("HOME") {
        return PathBuf::from(home).join(".local").join("share").join("ProCard");
    }
    std::env::temp_dir().join("ProCard")
}

/// A small procedurally-drawn app icon (an original card glyph with an "i").
pub fn load_app_icon() -> egui::IconData {
    let (w, h) = (64u32, 64u32);
    let mut rgba = vec![0u8; (w * h * 4) as usize];
    for y in 0..h {
        for x in 0..w {
            let i = ((y * w + x) * 4) as usize;
            // Rounded-rect card.
            let inside = x >= 6 && x < 58 && y >= 14 && y < 50;
            let corner = {
                let cx = x as i32;
                let cy = y as i32;
                let near = |px: i32, py: i32| ((cx - px).pow(2) + (cy - py).pow(2)) < 36;
                (x < 12 && y < 20 && !near(12, 20))
                    || (x >= 52 && y < 20 && !near(51, 20))
                    || (x < 12 && y >= 44 && !near(12, 49))
                    || (x >= 52 && y >= 44 && !near(51, 49))
            };
            if inside && !corner {
                // teal→indigo gradient
                let t = x as f32 / w as f32;
                let r = (40.0 + 20.0 * t) as u8;
                let g = (170.0 - 60.0 * t) as u8;
                let b = (190.0 + 40.0 * t) as u8;
                rgba[i] = r;
                rgba[i + 1] = g;
                rgba[i + 2] = b;
                rgba[i + 3] = 255;
            }
            // white "i"
            let dot = x >= 30 && x < 34 && y >= 20 && y < 24;
            let stem = x >= 30 && x < 34 && y >= 27 && y < 44;
            if dot || stem {
                rgba[i] = 245;
                rgba[i + 1] = 248;
                rgba[i + 2] = 252;
                rgba[i + 3] = 255;
            }
        }
    }
    egui::IconData {
        rgba,
        width: w,
        height: h,
    }
}

#[derive(PartialEq, Eq, Clone, Copy)]
pub(crate) enum Tab {
    Studio,
    Passcode,
    Logs,
}

/// Messages sent from worker threads back to the UI.
enum UiMsg {
    Log(String),
    Devices(Vec<DeviceInfo>),
    ScanHit(scanner::ScanHit),
    ScanEnded(String),
    OpFinished { ok: bool, message: String },
    Busy(bool),
}

pub struct IcardApp {
    ctx: egui::Context,
    settings: Settings,
    activation: Activation,

    tab: Tab,
    logs_unread: bool,

    // device state
    devices: Vec<DeviceInfo>,
    selected_device: usize,
    support: SupportStatus,

    // wallet / scan
    hash_input: String,
    saved: Vec<SavedCard>,
    scanning: bool,
    scan_stop: Arc<AtomicBool>,

    // image
    source: Option<DynamicImage>,
    work: Option<DynamicImage>, // downscaled copy for cheap preview cropping
    source_name: String,
    crop_offset: Vec2,
    preview_tex: Option<egui::TextureHandle>,
    preview_dirty: bool,

    zoom: f32,

    // passcode
    theme: Option<passthm::PasscodeTheme>,

    // catalog / artwork browser
    catalog: crate::catalog::Catalog,
    selected_pack: usize,
    selected_art: Option<String>,
    art_search: String,
    art_thumbs: std::collections::HashMap<String, egui::TextureHandle>,

    // misc UI
    logs: Vec<String>,
    status: String,
    busy: bool,
    show_activation: bool,
    activation_input: String,
    activation_msg: String,
    show_help: bool,

    tx: Sender<UiMsg>,
    rx: Receiver<UiMsg>,
    device_refresh_started: bool,
}

impl IcardApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let settings = saved_cards::load_settings();
        let activation = Activation::load();
        let saved = saved_cards::load_cards();
        let support = device::apple::detect();
        let (tx, rx) = std::sync::mpsc::channel();

        let catalog = crate::catalog::Catalog::load();

        let mut logs = Vec::new();
        logs.push(format!("ProCard {} initialized", env!("CARGO_PKG_VERSION")));
        logs.push(support.describe());
        logs.push(format!("Loaded {} saved card(s) from database", saved.len()));
        logs.push(format!(
            "Catalog: {} pack(s), {} artwork(s)",
            catalog.packs.len(),
            catalog.total_arts()
        ));

        let status = Strings::new(settings.language).ready().to_string();

        IcardApp {
            ctx: cc.egui_ctx.clone(),
            settings,
            activation,
            tab: Tab::Studio,
            logs_unread: false,
            devices: Vec::new(),
            selected_device: 0,
            support,
            hash_input: String::new(),
            saved,
            scanning: false,
            scan_stop: Arc::new(AtomicBool::new(false)),
            source: None,
            work: None,
            source_name: String::new(),
            crop_offset: Vec2::new(0.5, 0.5),
            preview_tex: None,
            preview_dirty: false,
            zoom: 1.0,
            theme: None,
            catalog,
            selected_pack: 0,
            selected_art: None,
            art_search: String::new(),
            art_thumbs: std::collections::HashMap::new(),
            logs,
            status,
            busy: false,
            show_activation: false,
            activation_input: String::new(),
            activation_msg: String::new(),
            show_help: false,
            tx,
            rx,
            device_refresh_started: false,
        }
    }

    pub fn configure_fonts(ctx: &egui::Context) {
        // Keep the default font stack but bump the base size a touch for the
        // card-shop look of the original.
        use egui::{FontFamily, FontId, TextStyle};
        let mut style = (*ctx.style()).clone();
        style.text_styles = [
            (TextStyle::Heading, FontId::new(22.0, FontFamily::Proportional)),
            (TextStyle::Body, FontId::new(15.0, FontFamily::Proportional)),
            (TextStyle::Monospace, FontId::new(13.0, FontFamily::Monospace)),
            (TextStyle::Button, FontId::new(15.0, FontFamily::Proportional)),
            (TextStyle::Small, FontId::new(12.0, FontFamily::Proportional)),
        ]
        .into();
        style.spacing.item_spacing = egui::vec2(8.0, 8.0);
        style.spacing.button_padding = egui::vec2(10.0, 6.0);
        ctx.set_style(style);
    }

    fn strings(&self) -> Strings {
        Strings::new(self.settings.language)
    }

    fn log(&mut self, line: impl Into<String>) {
        let line = line.into();
        let stamp = time::OffsetDateTime::now_utc();
        self.logs
            .push(format!("[{:02}:{:02}:{:02}] {line}", stamp.hour(), stamp.minute(), stamp.second()));
        if self.tab != Tab::Logs {
            self.logs_unread = true;
        }
        if self.logs.len() > 2000 {
            let drop = self.logs.len() - 2000;
            self.logs.drain(0..drop);
        }
    }

    fn selected_device(&self) -> Option<&DeviceInfo> {
        self.devices.get(self.selected_device)
    }

    /// Gate check: device-mutating actions require activation.
    fn gated(&mut self) -> bool {
        if self.activation.is_active() {
            true
        } else {
            let s = self.strings();
            self.status = s.need_activation().to_string();
            self.log(self.strings().need_activation());
            self.show_activation = true;
            false
        }
    }

    // ---- worker spawns ----

    fn worker_logger(tx: Sender<UiMsg>, ctx: egui::Context) -> impl FnMut(&str) {
        move |line: &str| {
            let _ = tx.send(UiMsg::Log(line.to_string()));
            ctx.request_repaint();
        }
    }

    fn refresh_devices(&mut self) {
        let tx = self.tx.clone();
        let ctx = self.ctx.clone();
        let mode = self.settings.transport;
        let _ = tx.send(UiMsg::Log("Scanning for connected iOS devices via usbmuxd...".into()));
        std::thread::spawn(move || {
            match device::list_devices(mode) {
                Ok(list) => {
                    let _ = tx.send(UiMsg::Log(format!(
                        "Found {} connected device(s); transport mode: {:?}",
                        list.len(),
                        mode
                    )));
                    let _ = tx.send(UiMsg::Devices(list));
                }
                Err(e) => {
                    let _ = tx.send(UiMsg::Log(format!("Could not enumerate devices: {e}")));
                    let _ = tx.send(UiMsg::Devices(Vec::new()));
                }
            }
            ctx.request_repaint();
        });
    }

    fn start_scan(&mut self) {
        let Some(device) = self.selected_device().cloned() else {
            self.status = self.strings().select_device().to_string();
            return;
        };
        if !self.gated() {
            return;
        }
        self.scan_stop = Arc::new(AtomicBool::new(false));
        self.scanning = true;
        self.status = self.strings().scanning_syslog().to_string();
        self.log("Initiating syslog monitor session...");
        let tx = self.tx.clone();
        let ctx = self.ctx.clone();
        let stop = self.scan_stop.clone();
        std::thread::spawn(move || {
            scanner::run_scan(&device, stop, |ev| {
                match ev {
                    scanner::ScanEvent::Log(l) => {
                        let _ = tx.send(UiMsg::Log(l));
                    }
                    scanner::ScanEvent::Hit(h) => {
                        let _ = tx.send(UiMsg::ScanHit(scanner::ScanHit {
                            hash: h.hash,
                            name: h.name,
                        }));
                    }
                    scanner::ScanEvent::Finished(m) => {
                        let _ = tx.send(UiMsg::ScanEnded(m));
                    }
                    scanner::ScanEvent::Error(m) => {
                        let _ = tx.send(UiMsg::Log(m));
                        let _ = tx.send(UiMsg::ScanEnded("Syslog scanning stopped.".into()));
                    }
                }
                ctx.request_repaint();
            });
        });
    }

    fn stop_scan(&mut self) {
        self.scan_stop.store(true, Ordering::Relaxed);
        self.scanning = false;
        self.log("Syslog scanning stopped by user.");
    }

    fn apply_card_skin(&mut self) {
        let s = self.strings();
        if self.source.is_none() {
            self.status = s.choose_image_first().to_string();
            return;
        }
        let hash = self.hash_input.trim().to_string();
        if hash.is_empty() {
            self.status = s.enter_hash_first().to_string();
            return;
        }
        let Some(device) = self.selected_device().cloned() else {
            self.status = s.select_device().to_string();
            return;
        };
        if !self.gated() {
            return;
        }
        let offset = (self.crop_offset.x, self.crop_offset.y);
        let zoom = self.zoom;
        let source = self.source.clone().unwrap();
        let name = self.source_name.clone();
        self.busy = true;
        self.log("Initiating card flash...");
        self.log(format!("Starting card skin flash for hash: {hash}"));
        let tx = self.tx.clone();
        let ctx = self.ctx.clone();
        std::thread::spawn(move || {
            let _ = tx.send(UiMsg::Busy(true));
            let mut logger = Self::worker_logger(tx.clone(), ctx.clone());
            let result = (|| {
                logger("Writing card skin to iPhone...");
                let skin = image_skin::prepare(&source, offset, zoom, name)
                    .map_err(|e| anyhow::anyhow!("Could not prepare card skin: {e}"))?;
                airlift::write_card_skin(&device, &hash, &skin, &mut logger)
            })();
            match result {
                Ok(msg) => {
                    let _ = tx.send(UiMsg::OpFinished { ok: true, message: msg });
                }
                Err(e) => {
                    let _ = tx.send(UiMsg::OpFinished {
                        ok: false,
                        message: format!("Flash failed: {e}"),
                    });
                }
            }
            let _ = tx.send(UiMsg::Busy(false));
            ctx.request_repaint();
        });
    }

    fn restore_original(&mut self) {
        let s = self.strings();
        let hash = self.hash_input.trim().to_string();
        if hash.is_empty() {
            self.status = s.enter_hash_first().to_string();
            return;
        }
        let Some(device) = self.selected_device().cloned() else {
            self.status = s.select_device().to_string();
            return;
        };
        if !self.gated() {
            return;
        }
        self.busy = true;
        self.log("Restoring original card face....");
        let tx = self.tx.clone();
        let ctx = self.ctx.clone();
        std::thread::spawn(move || {
            let _ = tx.send(UiMsg::Busy(true));
            let mut logger = Self::worker_logger(tx.clone(), ctx.clone());
            let result = airlift::restore_original(&device, &hash, &mut logger);
            match result {
                Ok(msg) => {
                    let _ = tx.send(UiMsg::OpFinished { ok: true, message: msg });
                }
                Err(e) => {
                    let _ = tx.send(UiMsg::OpFinished {
                        ok: false,
                        message: format!("Restore failed: {e}"),
                    });
                }
            }
            let _ = tx.send(UiMsg::Busy(false));
            ctx.request_repaint();
        });
    }

    fn apply_theme(&mut self) {
        let s = self.strings();
        let Some(theme) = self.theme.clone() else {
            self.status = s.choose_theme_first().to_string();
            return;
        };
        let Some(device) = self.selected_device().cloned() else {
            self.status = s.select_device().to_string();
            return;
        };
        if !self.gated() {
            return;
        }
        let lang = self.settings.keypad_language;
        let bold = self.settings.generate_bold_variant;
        self.busy = true;
        self.log("Starting passcode theme flash...");
        let tx = self.tx.clone();
        let ctx = self.ctx.clone();
        std::thread::spawn(move || {
            let _ = tx.send(UiMsg::Busy(true));
            let mut logger = Self::worker_logger(tx.clone(), ctx.clone());
            let result = passthm::apply(&device, &theme, lang, bold, &mut logger);
            match result {
                Ok(msg) => {
                    let _ = tx.send(UiMsg::OpFinished { ok: true, message: msg });
                }
                Err(e) => {
                    let _ = tx.send(UiMsg::OpFinished {
                        ok: false,
                        message: format!("Theme flash failed: {e}"),
                    });
                }
            }
            let _ = tx.send(UiMsg::Busy(false));
            ctx.request_repaint();
        });
    }

    // ---- image handling ----

    fn load_image(&mut self, path: PathBuf) {
        match image_skin::load_source(&path) {
            Ok(img) => {
                self.source_name = path
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_default();
                // Downscale a working copy for cheap preview re-cropping.
                self.work = Some(img.thumbnail(1024, 1024));
                self.source = Some(img);
                self.crop_offset = Vec2::new(0.5, 0.5);
                self.zoom = 1.0;
                self.selected_art = None;
                self.preview_dirty = true;
                self.log(format!("Prepared card skin source: {}", self.source_name));
            }
            Err(e) => {
                self.log(format!("No image: {e}"));
                self.status = e.to_string();
            }
        }
    }

    fn rebuild_preview(&mut self) {
        let Some(work) = &self.work else { return };
        let canvas = image_skin::crop_to_canvas(work, (self.crop_offset.x, self.crop_offset.y), self.zoom);
        // Downscale for the texture.
        let disp = image::imageops::resize(&canvas, 640, 404, image::imageops::FilterType::Triangle);
        let color = egui::ColorImage::from_rgba_unmultiplied([640, 404], disp.as_raw());
        self.preview_tex =
            Some(self.ctx.load_texture("card-skin-source-preview", color, Default::default()));
        self.preview_dirty = false;
    }

    // ---- message pump ----

    fn drain_messages(&mut self) {
        while let Ok(msg) = self.rx.try_recv() {
            match msg {
                UiMsg::Log(l) => self.log(l),
                UiMsg::Devices(list) => {
                    self.devices = list;
                    if self.selected_device >= self.devices.len() {
                        self.selected_device = 0;
                    }
                }
                UiMsg::ScanHit(hit) => {
                    self.hash_input = hit.hash.clone();
                    let card = SavedCard {
                        hash: hit.hash,
                        name: hit.name,
                        captured_at: now_iso(),
                    };
                    saved_cards::upsert(&mut self.saved, card);
                    saved_cards::save_cards(&self.saved);
                    self.log("Card captured");
                    self.status = "Card captured".into();
                    self.scanning = false;
                    self.scan_stop.store(true, Ordering::Relaxed);
                }
                UiMsg::ScanEnded(m) => {
                    self.scanning = false;
                    self.log(m);
                }
                UiMsg::OpFinished { ok, message } => {
                    self.busy = false;
                    self.log(message.as_str());
                    self.status = message;
                    if ok {
                        self.ctx.request_repaint();
                    }
                }
                UiMsg::Busy(b) => self.busy = b,
            }
        }
    }
}

fn now_iso() -> String {
    let t = time::OffsetDateTime::now_utc();
    format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z",
        t.year(),
        t.month() as u8,
        t.day(),
        t.hour(),
        t.minute(),
        t.second()
    )
}

impl eframe::App for IcardApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if !self.device_refresh_started {
            self.device_refresh_started = true;
            self.refresh_devices();
        }
        self.drain_messages();
        if self.preview_dirty {
            self.rebuild_preview();
        }

        self.render(ctx);
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        self.scan_stop.store(true, Ordering::Relaxed);
        saved_cards::save_settings(&self.settings);
        saved_cards::save_cards(&self.saved);
    }
}

impl IcardApp {
    /// Load a catalog artwork as the active card-skin source.
    pub(crate) fn select_catalog_art(&mut self, art_id: &str) {
        let path = match self.catalog.find(art_id) {
            Some((_, art)) => self.catalog.art_path(art),
            None => return,
        };
        let name = art_id.to_string();
        match image_skin::load_source(&path) {
            Ok(img) => {
                self.source_name = name;
                self.work = Some(img.thumbnail(1024, 1024));
                self.source = Some(img);
                self.crop_offset = Vec2::new(0.5, 0.5);
                self.zoom = 1.0;
                self.selected_art = Some(art_id.to_string());
                self.preview_dirty = true;
                self.log(format!("Selected artwork: {art_id}"));
            }
            Err(e) => {
                self.log(format!("Could not load artwork {art_id}: {e}"));
            }
        }
    }

    /// Lazily build (and cache) a small thumbnail texture for a catalog art.
    pub(crate) fn art_thumb(&mut self, art_id: &str, path: &std::path::Path) -> Option<egui::TextureHandle> {
        if let Some(t) = self.art_thumbs.get(art_id) {
            return Some(t.clone());
        }
        let img = image::ImageReader::open(path).ok()?.with_guessed_format().ok()?.decode().ok()?;
        let thumb = img.thumbnail(320, 202).to_rgba8();
        let color = egui::ColorImage::from_rgba_unmultiplied(
            [thumb.width() as usize, thumb.height() as usize],
            thumb.as_raw(),
        );
        let tex = self.ctx.load_texture(format!("thumb-{art_id}"), color, Default::default());
        self.art_thumbs.insert(art_id.to_string(), tex.clone());
        Some(tex)
    }
}

// The redesigned studio UI.
mod studio;
