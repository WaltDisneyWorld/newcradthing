//! ProCard's studio interface: sidebar, guided step ribbon, searchable artwork
//! browser, large card preview with zoom + drag crop, passcode/logs panels, and
//! the HWID-bound License Manager. Icons are drawn or plain text (no emoji font
//! dependency), so nothing renders as tofu.

use std::path::PathBuf;

use egui::{Color32, CornerRadius, RichText, Sense, Stroke, StrokeKind, Vec2};

use super::{IcardApp, Tab};
use crate::license::{LicenseStatus, ADMIN_CODE};
use crate::model::{KeypadLanguage, Language, TransportMode};
use crate::wallet::airlift;

// ---- palette ----
const BG: Color32 = Color32::from_rgb(16, 18, 23);
const PANEL: Color32 = Color32::from_rgb(26, 30, 37);
const PANEL2: Color32 = Color32::from_rgb(34, 39, 48);
const CARD: Color32 = Color32::from_rgb(22, 25, 31);
const ACCENT: Color32 = Color32::from_rgb(64, 196, 214);
const ACCENT2: Color32 = Color32::from_rgb(132, 120, 240);
const TEXT: Color32 = Color32::from_rgb(228, 232, 238);
const MUTED: Color32 = Color32::from_rgb(150, 158, 170);
const GOOD: Color32 = Color32::from_rgb(90, 200, 130);
const WARN: Color32 = Color32::from_rgb(230, 170, 90);
const BAD: Color32 = Color32::from_rgb(220, 110, 110);

impl IcardApp {
    fn tr(&self, en: &'static str, it: &'static str) -> &'static str {
        match self.settings.language {
            Language::Italian => it,
            _ => en,
        }
    }

    pub(super) fn render(&mut self, ctx: &egui::Context) {
        self.install_theme(ctx);

        egui::SidePanel::left("nav")
            .exact_width(252.0)
            .resizable(false)
            .frame(egui::Frame::new().fill(PANEL).inner_margin(16))
            .show(ctx, |ui| self.render_sidebar(ui));

        egui::TopBottomPanel::bottom("status")
            .frame(egui::Frame::new().fill(PANEL).inner_margin(egui::Margin::symmetric(16, 6)))
            .show(ctx, |ui| self.render_status(ui));

        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(BG).inner_margin(18))
            .show(ctx, |ui| match self.tab {
                Tab::Studio => self.render_studio(ui),
                Tab::Passcode => self.render_passcode(ui),
                Tab::Logs => self.render_logs(ui),
            });

        self.render_help(ctx);
        self.render_activation(ctx);
        self.render_manager(ctx);
    }

    fn install_theme(&self, ctx: &egui::Context) {
        if ctx.style().visuals.panel_fill == BG {
            return;
        }
        let mut v = egui::Visuals::dark();
        v.panel_fill = BG;
        v.window_fill = PANEL;
        v.extreme_bg_color = CARD;
        v.faint_bg_color = PANEL2;
        v.override_text_color = Some(TEXT);
        v.selection.bg_fill = ACCENT.linear_multiply(0.45);
        v.selection.stroke = Stroke::new(1.0_f32, ACCENT);
        v.widgets.inactive.bg_fill = PANEL2;
        v.widgets.inactive.weak_bg_fill = PANEL2;
        v.widgets.hovered.bg_fill = Color32::from_rgb(44, 50, 60);
        v.widgets.hovered.weak_bg_fill = Color32::from_rgb(44, 50, 60);
        v.widgets.active.bg_fill = ACCENT.linear_multiply(0.5);
        v.widgets.noninteractive.bg_stroke = Stroke::new(1.0_f32, Color32::from_rgb(44, 50, 60));
        for w in [&mut v.widgets.inactive, &mut v.widgets.hovered, &mut v.widgets.active] {
            w.corner_radius = CornerRadius::same(8);
        }
        v.window_corner_radius = CornerRadius::same(12);
        ctx.set_visuals(v);
    }

    // ---------- sidebar ----------

    fn render_sidebar(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            logo_mark(ui);
            ui.add_space(4.0);
            ui.vertical(|ui| {
                ui.label(RichText::new("ProCard").size(22.0).strong().color(TEXT));
                ui.label(RichText::new(format!("Wallet Studio · v{}", env!("CARGO_PKG_VERSION"))).size(11.0).color(MUTED));
            });
        });
        ui.add_space(14.0);

        let studio = self.tr("Card Studio", "Studio Carte");
        let passcode = self.tr("Passcode Theme", "Tema Tastierino");
        let logs = self.tr("Activity Log", "Registro");
        if nav_item(ui, studio, matches!(self.tab, Tab::Studio), false) {
            self.tab = Tab::Studio;
        }
        if nav_item(ui, passcode, matches!(self.tab, Tab::Passcode), false) {
            self.tab = Tab::Passcode;
        }
        if nav_item(ui, logs, matches!(self.tab, Tab::Logs), self.logs_unread) {
            self.tab = Tab::Logs;
            self.logs_unread = false;
        }

        ui.add_space(12.0);
        section_rule(ui);
        ui.add_space(10.0);

        ui.label(RichText::new(self.tr("DEVICE", "DISPOSITIVO")).size(11.0).strong().color(MUTED));
        ui.add_space(6.0);
        self.device_card(ui);

        ui.add_space(14.0);
        ui.label(RichText::new(self.tr("YOUR CARDS", "LE TUE CARTE")).size(11.0).strong().color(MUTED));
        ui.add_space(6.0);
        self.saved_cards_list(ui);

        ui.with_layout(egui::Layout::bottom_up(egui::Align::LEFT), |ui| {
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                let mut lang = self.settings.language;
                lang_toggle(ui, &mut lang);
                if lang != self.settings.language {
                    self.settings.language = lang;
                    self.status = crate::i18n::Strings::new(lang).ready().to_string();
                    crate::saved_cards::save_settings(&self.settings);
                }
                ui.add_space(6.0);
                if ui.add(text_button(self.tr("Help", "Aiuto"), PANEL2)).clicked() {
                    self.show_help = true;
                }
            });
            ui.add_space(6.0);
            // License status / manager
            ui.horizontal(|ui| {
                if self.license_ok() {
                    chip(ui, self.tr("Licensed", "Con licenza"), GOOD);
                } else {
                    chip(ui, self.tr("Not licensed", "Senza licenza"), WARN);
                }
                if ui.add(text_button(self.tr("License…", "Licenza…"), ACCENT2.linear_multiply(0.55))).clicked() {
                    self.show_manager = true;
                }
            });
        });
    }

    fn device_card(&mut self, ui: &mut egui::Ui) {
        egui::Frame::new().fill(PANEL2).corner_radius(10).inner_margin(10).show(ui, |ui| {
            ui.set_width(ui.available_width());
            let has = self.selected_device().is_some();
            ui.horizontal(|ui| {
                let (rect, _) = ui.allocate_exact_size(Vec2::splat(10.0), Sense::hover());
                ui.painter().circle_filled(rect.center(), 5.0, if has { GOOD } else { Color32::from_gray(90) });
                let name = self
                    .selected_device()
                    .map(|d| d.label())
                    .unwrap_or_else(|| self.tr("No iPhone connected", "Nessun iPhone").to_string());
                ui.label(RichText::new(name).color(if has { TEXT } else { MUTED }));
            });
            if let Some(d) = self.selected_device() {
                if !d.ios_version.is_empty() {
                    ui.label(RichText::new(format!("iOS {}", d.ios_version)).size(11.0).color(MUTED));
                }
            }
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                let mut mode = self.settings.transport;
                egui::ComboBox::from_id_salt("transport")
                    .width(120.0)
                    .selected_text(RichText::new(transport_label(self, mode)).size(12.0))
                    .show_ui(ui, |ui| {
                        for m in TransportMode::all() {
                            ui.selectable_value(&mut mode, m, transport_label(self, m));
                        }
                    });
                if mode != self.settings.transport {
                    self.settings.transport = mode;
                    crate::saved_cards::save_settings(&self.settings);
                    self.refresh_devices();
                }
                if ui.add(text_button(self.tr("Rescan", "Cerca"), PANEL)).clicked() {
                    self.refresh_devices();
                }
            });
            if self.devices.len() > 1 {
                let mut sel = self.selected_device;
                egui::ComboBox::from_id_salt("devpick")
                    .width(ui.available_width())
                    .selected_text(self.selected_device().map(|d| d.label()).unwrap_or_default())
                    .show_ui(ui, |ui| {
                        for (i, d) in self.devices.iter().enumerate() {
                            ui.selectable_value(&mut sel, i, d.label());
                        }
                    });
                self.selected_device = sel;
            }
        });
    }

    fn saved_cards_list(&mut self, ui: &mut egui::Ui) {
        egui::ScrollArea::vertical().id_salt("saved").max_height(150.0).auto_shrink([false, true]).show(ui, |ui| {
            if self.saved.is_empty() {
                ui.label(RichText::new(self.tr("Scan a card to save it here.", "Scansiona una carta per salvarla.")).size(12.0).color(MUTED));
            }
            let mut pick = None;
            for c in &self.saved {
                let selected = self.hash_input == c.hash;
                let label = if c.name.is_empty() { crate::model::short_udid(&c.hash) } else { c.name.clone() };
                if ui.add(card_row(&label, &crate::model::short_udid(&c.hash), selected)).clicked() {
                    pick = Some(c.hash.clone());
                }
            }
            if let Some(h) = pick {
                self.hash_input = h;
            }
        });
    }

    // ---------- studio ----------

    fn render_studio(&mut self, ui: &mut egui::Ui) {
        self.step_ribbon(ui);
        ui.add_space(14.0);
        let total_w = ui.available_width();
        let left_w = (total_w * 0.52).clamp(360.0, 680.0);
        ui.horizontal_top(|ui| {
            ui.allocate_ui(Vec2::new(left_w, ui.available_height()), |ui| self.render_preview_pane(ui));
            ui.add_space(14.0);
            ui.allocate_ui(Vec2::new(ui.available_width(), ui.available_height()), |ui| self.render_browser(ui));
        });
    }

    fn step_ribbon(&mut self, ui: &mut egui::Ui) {
        let device_ok = self.selected_device().is_some();
        let hash_ok = !self.hash_input.trim().is_empty();
        let art_ok = self.source.is_some();
        let steps = [
            (self.tr("Connect", "Collega"), device_ok),
            (self.tr("Scan card", "Scansiona"), hash_ok),
            (self.tr("Choose artwork", "Scegli grafica"), art_ok),
            (self.tr("Apply", "Applica"), false),
        ];
        egui::Frame::new().fill(PANEL).corner_radius(12).inner_margin(12).show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                for (i, (label, done)) in steps.iter().enumerate() {
                    step_pill(ui, i + 1, label, *done);
                    if i < steps.len() - 1 {
                        step_sep(ui);
                    }
                }
            });
        });
    }

    fn render_preview_pane(&mut self, ui: &mut egui::Ui) {
        egui::Frame::new().fill(PANEL).corner_radius(14).inner_margin(16).show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.label(RichText::new(self.tr("Card preview", "Anteprima carta")).strong().size(15.0));
            ui.label(RichText::new(self.tr("1536 × 969 px · drag to reposition", "1536 × 969 px · trascina per spostare")).size(11.0).color(MUTED));
            ui.add_space(8.0);

            let avail = ui.available_width();
            let size = Vec2::new(avail, avail * (969.0_f32 / 1536.0));
            let (rect, resp) = ui.allocate_exact_size(size, Sense::drag());
            let painter = ui.painter_at(rect);
            painter.rect_filled(rect, CornerRadius::same(16), CARD);
            if let Some(tex) = &self.preview_tex {
                egui::Image::new(egui::load::SizedTexture::new(tex.id(), rect.size())).corner_radius(16).paint_at(ui, rect);
            } else {
                painter.text(rect.center(), egui::Align2::CENTER_CENTER, self.tr("Pick artwork or upload an image", "Scegli una grafica o carica un'immagine"), egui::FontId::proportional(15.0), MUTED);
            }
            painter.rect_stroke(rect, CornerRadius::same(16), Stroke::new(1.0_f32, Color32::from_gray(60)), StrokeKind::Inside);
            if resp.dragged() && self.work.is_some() {
                let d = resp.drag_delta();
                self.crop_offset.x = (self.crop_offset.x - d.x / rect.width()).clamp(0.0, 1.0);
                self.crop_offset.y = (self.crop_offset.y - d.y / rect.height()).clamp(0.0, 1.0);
                self.preview_dirty = true;
            }

            if self.source.is_some() {
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    ui.label(RichText::new(self.tr("Zoom", "Zoom")).size(12.0).color(MUTED));
                    let mut z = self.zoom;
                    if ui.add(egui::Slider::new(&mut z, 1.0..=3.0).show_value(false)).changed() {
                        self.zoom = z;
                        self.preview_dirty = true;
                    }
                    if ui.add(text_button(self.tr("Reset", "Reimposta"), PANEL2)).clicked() {
                        self.zoom = 1.0;
                        self.crop_offset = Vec2::new(0.5, 0.5);
                        self.preview_dirty = true;
                    }
                });
            }

            ui.add_space(10.0);
            section_rule(ui);
            ui.add_space(10.0);

            ui.label(RichText::new(self.tr("Target card", "Carta di destinazione")).size(12.0).color(MUTED));
            let hash_hint = self.tr("Scan or paste the pass hash", "Scansiona o incolla l'hash");
            ui.horizontal(|ui| {
                let w = ui.available_width() - 92.0;
                ui.add(egui::TextEdit::singleline(&mut self.hash_input).hint_text(hash_hint).desired_width(w));
                if self.scanning {
                    if ui.add(danger_button(self.tr("Stop", "Ferma"))).clicked() {
                        self.stop_scan();
                    }
                } else if ui.add(text_button(self.tr("Scan", "Cerca"), ACCENT2.linear_multiply(0.6))).clicked() {
                    self.start_scan();
                }
            });
            if self.scanning {
                ui.horizontal(|ui| {
                    ui.spinner();
                    ui.label(RichText::new(self.tr("Open Wallet and tap your card…", "Apri Wallet e tocca la carta…")).size(12.0).italics().color(MUTED));
                });
            }

            ui.add_space(12.0);
            let can_apply = !self.busy && self.source.is_some() && !self.hash_input.trim().is_empty() && self.selected_device().is_some();
            let can_restore = !self.busy && airlift::restore_available(&self.hash_input) && self.selected_device().is_some();
            ui.horizontal(|ui| {
                if ui.add_enabled(can_apply, primary_button(self.tr("Apply to iPhone", "Applica all'iPhone"))).clicked() {
                    self.apply_card_skin();
                }
                if ui.add_enabled(can_restore, text_button(self.tr("Restore original", "Ripristina"), PANEL2)).clicked() {
                    self.restore_original();
                }
            });
            ui.label(RichText::new(self.tr("After applying, force-close Wallet and reopen it.", "Dopo l'applicazione, chiudi e riapri Wallet.")).size(11.0).color(MUTED));
        });
    }

    fn render_browser(&mut self, ui: &mut egui::Ui) {
        egui::Frame::new().fill(PANEL).corner_radius(14).inner_margin(16).show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.set_min_height(ui.available_height());
            ui.horizontal(|ui| {
                ui.label(RichText::new(self.tr("Artwork", "Grafiche")).strong().size(15.0));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.add(text_button(self.tr("Upload image", "Carica"), ACCENT.linear_multiply(0.45))).clicked() {
                        if let Some(path) = rfd::FileDialog::new().add_filter("Images", &["png", "jpg", "jpeg", "webp"]).pick_file() {
                            self.load_image(path);
                        }
                    }
                });
            });
            ui.add_space(6.0);

            if self.catalog.is_empty() {
                ui.add_space(20.0);
                ui.vertical_centered(|ui| {
                    ui.label(RichText::new(self.tr("No artwork packs found.", "Nessun pacchetto trovato.")).color(MUTED));
                    ui.label(RichText::new(self.tr("Add packs under assets/artwork, or Upload your own image.", "Aggiungi pacchetti in assets/artwork, oppure carica un'immagine.")).size(11.0).color(MUTED));
                });
                return;
            }

            let search_hint = self.tr("Search artwork…", "Cerca grafiche…");
            let search_w = ui.available_width();
            ui.add(egui::TextEdit::singleline(&mut self.art_search).hint_text(search_hint).desired_width(search_w));
            ui.add_space(8.0);

            let searching = !self.art_search.trim().is_empty();
            if !searching {
                let packs: Vec<(usize, String, Color32)> = self
                    .catalog
                    .packs
                    .iter()
                    .enumerate()
                    .map(|(i, p)| (i, p.display_name(self.settings.language).to_string(), p.accent_color()))
                    .collect();
                ui.horizontal_wrapped(|ui| {
                    for (i, name, accent) in packs {
                        if ui.add(pack_chip(&name, accent, self.selected_pack == i)).clicked() {
                            self.selected_pack = i;
                        }
                    }
                });
                ui.add_space(10.0);
            }

            let query = self.art_search.trim().to_lowercase();
            let mut items: Vec<(String, String, PathBuf)> = Vec::new();
            for (pi, pack) in self.catalog.packs.iter().enumerate() {
                if !searching && pi != self.selected_pack {
                    continue;
                }
                for art in &pack.arts {
                    if searching {
                        let hay = format!("{} {}", art.name, pack.name).to_lowercase();
                        if !hay.contains(&query) {
                            continue;
                        }
                    }
                    items.push((art.id.clone(), art.name.clone(), self.catalog.art_path(art)));
                }
            }

            egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                let col_w = 150.0;
                let cols = (ui.available_width() / (col_w + 10.0)).floor().max(1.0) as usize;
                let thumb = Vec2::new(col_w, col_w * (969.0_f32 / 1536.0));
                egui::Grid::new("artgrid").spacing([10.0, 12.0]).show(ui, |ui| {
                    for (n, (id, name, path)) in items.iter().enumerate() {
                        ui.vertical(|ui| {
                            let selected = self.selected_art.as_deref() == Some(id.as_str());
                            let tex = self.art_thumb(id, path);
                            let (rect, resp) = ui.allocate_exact_size(thumb, Sense::click());
                            let p = ui.painter_at(rect);
                            p.rect_filled(rect, CornerRadius::same(10), CARD);
                            if let Some(tex) = tex {
                                egui::Image::new(egui::load::SizedTexture::new(tex.id(), rect.size())).corner_radius(10).paint_at(ui, rect);
                            }
                            let stroke = if selected { Stroke::new(2.5_f32, ACCENT) } else { Stroke::new(1.0_f32, Color32::from_gray(55)) };
                            p.rect_stroke(rect, CornerRadius::same(10), stroke, StrokeKind::Outside);
                            if resp.clicked() {
                                self.select_catalog_art(id);
                            }
                            ui.add_space(2.0);
                            ui.label(RichText::new(name).size(11.0).color(if selected { ACCENT } else { MUTED }));
                        });
                        if (n + 1) % cols == 0 {
                            ui.end_row();
                        }
                    }
                });
                if items.is_empty() {
                    ui.add_space(16.0);
                    ui.vertical_centered(|ui| ui.label(RichText::new(self.tr("No matches.", "Nessun risultato.")).color(MUTED)));
                }
            });
        });
    }

    // ---------- passcode ----------

    fn render_passcode(&mut self, ui: &mut egui::Ui) {
        egui::Frame::new().fill(PANEL).corner_radius(14).inner_margin(18).show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.label(RichText::new(self.tr("Passcode keypad theme", "Tema del tastierino")).strong().size(17.0));
            ui.label(RichText::new(self.tr("Custom lockscreen keypad from a .passthm package (Cowabunga / Nugget format).", "Tastierino personalizzato da un pacchetto .passthm (formato Cowabunga / Nugget).")).size(12.0).color(MUTED));
            ui.add_space(12.0);
            ui.columns(2, |cols| {
                let ui = &mut cols[0];
                if ui.add(primary_button(self.tr("Choose .passthm…", "Scegli .passthm…"))).clicked() {
                    if let Some(path) = rfd::FileDialog::new().add_filter("Passcode theme", &["passthm", "zip"]).pick_file() {
                        match crate::passthm::load(&path) {
                            Ok(t) => {
                                self.log(format!("Passcode theme loaded: '{}' ({} buttons)", t.name, t.button_count()));
                                self.theme = Some(t);
                            }
                            Err(e) => {
                                self.log(format!("Failed to load theme: {e}"));
                                self.status = e.to_string();
                            }
                        }
                    }
                }
                let label = self.theme.as_ref().map(|t| format!("{} · {} buttons", t.name, t.button_count())).unwrap_or_else(|| "—".into());
                ui.label(RichText::new(label).size(12.0).color(MUTED));

                ui.add_space(12.0);
                ui.label(RichText::new(self.tr("Keypad language", "Lingua tastierino")).size(12.0).color(MUTED));
                let mut lang = self.settings.keypad_language;
                egui::ComboBox::from_id_salt("keypad_lang").selected_text(keypad_label(lang)).show_ui(ui, |ui| {
                    for k in KeypadLanguage::all() {
                        ui.selectable_value(&mut lang, k, keypad_label(k));
                    }
                });
                if lang != self.settings.keypad_language {
                    self.settings.keypad_language = lang;
                    crate::saved_cards::save_settings(&self.settings);
                }
                let mut bold = self.settings.generate_bold_variant;
                if ui.checkbox(&mut bold, self.tr("Generate bold variants", "Genera varianti in grassetto")).changed() {
                    self.settings.generate_bold_variant = bold;
                    crate::saved_cards::save_settings(&self.settings);
                }

                ui.add_space(14.0);
                let can = !self.busy && self.theme.is_some() && self.selected_device().is_some();
                if ui.add_enabled(can, primary_button(self.tr("Apply theme", "Applica tema"))).clicked() {
                    self.apply_theme();
                }
                ui.label(RichText::new(self.tr("After applying, lock your iPhone to see the new keypad.", "Dopo l'applicazione, blocca l'iPhone per vedere il tastierino.")).size(11.0).color(MUTED));

                let ui = &mut cols[1];
                ui.label(RichText::new(self.tr("Keypad preview", "Anteprima tastierino")).strong());
                keypad_preview(ui, self.theme.as_ref());
            });
        });
    }

    // ---------- logs ----------

    fn render_logs(&mut self, ui: &mut egui::Ui) {
        egui::Frame::new().fill(PANEL).corner_radius(14).inner_margin(16).show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.set_min_height(ui.available_height());
            ui.horizontal(|ui| {
                ui.label(RichText::new(self.tr("Activity log", "Registro attività")).strong().size(15.0));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.add(text_button(self.tr("Clear", "Pulisci"), PANEL2)).clicked() {
                        self.logs.clear();
                    }
                    if ui.add(text_button(self.tr("Save", "Salva"), PANEL2)).clicked() {
                        if let Some(path) = rfd::FileDialog::new().set_file_name("procard-diagnostics.log").add_filter("Log", &["log", "txt"]).save_file() {
                            let _ = std::fs::write(path, self.logs.join("\n"));
                        }
                    }
                    if ui.add(text_button(self.tr("Copy", "Copia"), PANEL2)).clicked() {
                        ui.ctx().copy_text(self.logs.join("\n"));
                    }
                });
            });
            ui.add_space(8.0);
            egui::Frame::new().fill(CARD).corner_radius(10).inner_margin(10).show(ui, |ui| {
                egui::ScrollArea::vertical().auto_shrink([false, false]).stick_to_bottom(true).show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    if self.logs.is_empty() {
                        ui.label(RichText::new(self.tr("No events yet.", "Nessun evento.")).color(MUTED));
                    }
                    for line in &self.logs {
                        ui.label(RichText::new(line).monospace().size(12.0).color(TEXT));
                    }
                });
            });
        });
    }

    fn render_status(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            if self.busy {
                ui.spinner();
            }
            ui.label(RichText::new(&self.status).size(12.0).color(TEXT));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let c = if self.support.is_ready() { GOOD } else { MUTED };
                ui.label(RichText::new(self.support.describe()).size(11.0).color(c));
            });
        });
    }

    // ---------- modals ----------

    fn render_help(&mut self, ctx: &egui::Context) {
        if !self.show_help {
            return;
        }
        let mut open = true;
        egui::Window::new(self.tr("Setup & help", "Configurazione e aiuto"))
            .collapsible(false).resizable(false).anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0]).open(&mut open)
            .show(ctx, |ui| {
                ui.set_max_width(460.0);
                ui.label(RichText::new(self.support.describe()).size(12.0).color(if self.support.is_ready() { GOOD } else { WARN }));
                ui.add_space(6.0);
                ui.label(self.tr(HELP_EN, HELP_IT));
            });
        if !open {
            self.show_help = false;
        }
    }

    fn render_activation(&mut self, ctx: &egui::Context) {
        if !self.show_activation {
            return;
        }
        let mut open = true;
        egui::Window::new(self.tr("Activate license", "Attiva licenza"))
            .collapsible(false).resizable(false).anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0]).open(&mut open)
            .show(ctx, |ui| {
                ui.set_max_width(360.0);
                ui.label(RichText::new(format!("{}: {}", self.tr("This device", "Questo dispositivo"), self.hwid)).size(11.0).color(MUTED));
                ui.add_space(4.0);
                ui.label(RichText::new(self.tr("Enter your license code", "Inserisci il codice di licenza")).size(12.0).color(MUTED));
                ui.add(egui::TextEdit::singleline(&mut self.activation_input).hint_text("XXXX-XXXX-XXXX-XXXX").desired_width(280.0));
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    if ui.add(primary_button(self.tr("Activate", "Attiva"))).clicked() {
                        self.do_activate();
                    }
                    if ui.add(text_button(self.tr("License Manager…", "Gestione licenze…"), PANEL2)).clicked() {
                        self.show_manager = true;
                        self.show_activation = false;
                    }
                });
                if !self.activation_msg.is_empty() {
                    let ok = self.license_ok();
                    ui.label(RichText::new(&self.activation_msg).color(if ok { GOOD } else { BAD }));
                }
            });
        if !open {
            self.show_activation = false;
        }
    }

    fn do_activate(&mut self) {
        let code = self.activation_input.clone();
        match self.licenses.activate(&code, &self.hwid) {
            Ok(_) => {
                self.activation.activated = true;
                self.activation.code = crate::activation::normalize(&code);
                self.activation.save();
                self.activation_msg = self.tr("Activation successful.", "Attivazione riuscita.").to_string();
                self.show_activation = false;
                self.log("License activated and bound to this device (HWID).");
            }
            Err(e) => {
                self.activation_msg = e;
            }
        }
    }

    fn render_manager(&mut self, ctx: &egui::Context) {
        if !self.show_manager {
            return;
        }
        let mut open = true;
        egui::Window::new(self.tr("License Manager", "Gestione licenze"))
            .collapsible(false).resizable(true).default_width(560.0).anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0]).open(&mut open)
            .show(ctx, |ui| {
                ui.set_max_width(640.0);

                // HWID + current status
                egui::Frame::new().fill(PANEL2).corner_radius(10).inner_margin(10).show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(self.tr("This machine HWID", "HWID di questo PC")).size(12.0).color(MUTED));
                        ui.label(RichText::new(&self.hwid).monospace().color(ACCENT));
                        if ui.add(text_button(self.tr("Copy", "Copia"), PANEL)).clicked() {
                            ui.ctx().copy_text(self.hwid.clone());
                        }
                    });
                    let licensed = self.license_ok();
                    ui.horizontal(|ui| {
                        chip(ui, if licensed { self.tr("Licensed on this device", "Con licenza su questo PC") } else { self.tr("Not licensed here", "Nessuna licenza qui") }, if licensed { GOOD } else { WARN });
                        if licensed {
                            ui.label(RichText::new(crate::license::group(&self.activation.code)).monospace().size(12.0).color(MUTED));
                        }
                    });
                });

                ui.add_space(8.0);

                // Activation row
                egui::Frame::new().fill(CARD).corner_radius(10).inner_margin(10).show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.label(RichText::new(self.tr("Activate a license on this device", "Attiva una licenza su questo PC")).size(12.0).strong());
                    ui.horizontal(|ui| {
                        ui.add(egui::TextEdit::singleline(&mut self.activation_input).hint_text("XXXX-XXXX-XXXX-XXXX").desired_width(240.0));
                        if ui.add(primary_button(self.tr("Activate", "Attiva"))).clicked() {
                            self.do_activate();
                        }
                        if self.license_ok() && ui.add(danger_button(self.tr("Reset my HWID", "Reimposta HWID"))).clicked() {
                            let code = self.activation.code.clone();
                            self.licenses.reset_hwid(&code);
                            self.activation.activated = false;
                            self.activation.save();
                            self.log("HWID reset for the active license.");
                            self.status = self.tr("HWID reset. Re-activate to bind again.", "HWID reimpostato. Riattiva per associare.").to_string();
                        }
                    });
                    if !self.activation_msg.is_empty() {
                        ui.label(RichText::new(&self.activation_msg).size(12.0).color(if self.license_ok() { GOOD } else { BAD }));
                    }
                });

                ui.add_space(8.0);
                section_rule(ui);
                ui.add_space(8.0);

                // Admin area
                if !self.manager_unlocked {
                    ui.label(RichText::new(self.tr("Admin — unlock to generate & manage licenses", "Admin — sblocca per generare e gestire")).size(12.0).strong());
                    let admin_hint = self.tr("Admin code", "Codice admin");
                    ui.horizontal(|ui| {
                        ui.add(egui::TextEdit::singleline(&mut self.admin_input).hint_text(admin_hint).desired_width(220.0));
                        if ui.add(primary_button(self.tr("Unlock", "Sblocca"))).clicked() {
                            if crate::activation::normalize(&self.admin_input) == ADMIN_CODE {
                                self.manager_unlocked = true;
                                self.log("License Manager unlocked (admin).");
                            } else {
                                self.status = self.tr("Invalid admin code.", "Codice admin non valido.").to_string();
                            }
                        }
                    });
                } else {
                    self.render_admin_table(ui);
                }
            });
        if !open {
            self.show_manager = false;
        }
    }

    fn render_admin_table(&mut self, ui: &mut egui::Ui) {
        let gen_hint = self.tr("Label (optional)", "Etichetta");
        ui.horizontal(|ui| {
            ui.label(RichText::new(self.tr("Generate new license", "Genera nuova licenza")).size(12.0).strong());
            ui.add(egui::TextEdit::singleline(&mut self.gen_label).hint_text(gen_hint).desired_width(180.0));
            if ui.add(primary_button(self.tr("Generate", "Genera"))).clicked() {
                let label = self.gen_label.clone();
                let key = self.licenses.generate(&label);
                self.gen_label.clear();
                self.log(format!("Generated license {}", crate::license::group(&key)));
            }
        });
        ui.add_space(8.0);

        // snapshot to avoid borrow clashes
        let hwid = self.hwid.clone();
        let records: Vec<crate::license::LicenseRecord> = self.licenses.records.clone();
        ui.label(RichText::new(format!("{} {}", records.len(), self.tr("license(s)", "licenze"))).size(11.0).color(MUTED));
        ui.add_space(4.0);

        enum Act {
            Reset(String),
            Revoke(String, bool),
            Delete(String),
            Copy(String),
        }
        let mut action: Option<Act> = None;

        egui::ScrollArea::vertical().max_height(260.0).auto_shrink([false, false]).show(ui, |ui| {
            egui::Grid::new("lictable").num_columns(4).spacing([10.0, 8.0]).striped(true).show(ui, |ui| {
                ui.label(RichText::new(self.tr("KEY", "CHIAVE")).size(10.5).strong().color(MUTED));
                ui.label(RichText::new(self.tr("LABEL", "ETICHETTA")).size(10.5).strong().color(MUTED));
                ui.label(RichText::new(self.tr("STATUS / HWID", "STATO / HWID")).size(10.5).strong().color(MUTED));
                ui.label(RichText::new(self.tr("ACTIONS", "AZIONI")).size(10.5).strong().color(MUTED));
                ui.end_row();

                for r in &records {
                    ui.label(RichText::new(crate::license::group(&r.key)).monospace().size(12.0).color(TEXT));
                    ui.label(RichText::new(&r.label).size(12.0).color(MUTED));
                    let (txt, col) = match r.status(&hwid) {
                        LicenseStatus::ActiveHere => (format!("{} · {}", tr_static(self, "Active here", "Attiva qui"), r.hwid.clone().unwrap_or_default()), GOOD),
                        LicenseStatus::BoundElsewhere => (format!("{} · {}", tr_static(self, "Bound", "Associata"), r.hwid.clone().unwrap_or_default()), WARN),
                        LicenseStatus::Revoked => (tr_static(self, "Revoked", "Revocata").to_string(), BAD),
                        LicenseStatus::Free => (tr_static(self, "Free", "Libera").to_string(), MUTED),
                    };
                    ui.label(RichText::new(txt).size(11.0).color(col));
                    ui.horizontal(|ui| {
                        if ui.add(text_button(self.tr("Copy", "Copia"), PANEL2)).clicked() {
                            action = Some(Act::Copy(r.key.clone()));
                        }
                        if ui.add(text_button(self.tr("Reset HWID", "Reset HWID"), PANEL2)).clicked() {
                            action = Some(Act::Reset(r.key.clone()));
                        }
                        let rv = if r.revoked { self.tr("Restore", "Ripristina") } else { self.tr("Revoke", "Revoca") };
                        if ui.add(text_button(rv, PANEL2)).clicked() {
                            action = Some(Act::Revoke(r.key.clone(), !r.revoked));
                        }
                        if ui.add(danger_button("X")).clicked() {
                            action = Some(Act::Delete(r.key.clone()));
                        }
                    });
                    ui.end_row();
                }
            });
        });

        match action {
            Some(Act::Reset(k)) => {
                self.licenses.reset_hwid(&k);
                if crate::activation::normalize(&self.activation.code) == k {
                    self.activation.activated = false;
                    self.activation.save();
                }
                self.log(format!("Reset HWID for {}", crate::license::group(&k)));
            }
            Some(Act::Revoke(k, v)) => {
                self.licenses.set_revoked(&k, v);
                self.log(format!("{} {}", if v { "Revoked" } else { "Restored" }, crate::license::group(&k)));
            }
            Some(Act::Delete(k)) => {
                self.licenses.delete(&k);
                self.log(format!("Deleted license {}", crate::license::group(&k)));
            }
            Some(Act::Copy(k)) => {
                ui.ctx().copy_text(crate::license::group(&k));
            }
            None => {}
        }
    }
}

// Free helper so the table can resolve IT/EN without borrowing issues.
fn tr_static(app: &IcardApp, en: &'static str, it: &'static str) -> &'static str {
    match app.settings.language {
        Language::Italian => it,
        _ => en,
    }
}

// ---------------- widget helpers ----------------

fn logo_mark(ui: &mut egui::Ui) {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(34.0, 34.0), Sense::hover());
    let p = ui.painter_at(rect);
    p.rect_filled(rect, CornerRadius::same(9), ACCENT);
    p.rect_filled(rect.shrink2(Vec2::new(7.0, 11.0)), CornerRadius::same(3), ACCENT2);
    p.text(rect.center(), egui::Align2::CENTER_CENTER, "P", egui::FontId::proportional(18.0), Color32::WHITE);
}

fn section_rule(ui: &mut egui::Ui) {
    let w = ui.available_width();
    let (rect, _) = ui.allocate_exact_size(Vec2::new(w, 1.0), Sense::hover());
    ui.painter().rect_filled(rect, CornerRadius::ZERO, Color32::from_gray(48));
}

fn nav_item(ui: &mut egui::Ui, label: &str, active: bool, badge: bool) -> bool {
    let fill = if active { ACCENT.linear_multiply(0.22) } else { Color32::TRANSPARENT };
    let text_col = if active { ACCENT } else { TEXT };
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 36.0), Sense::click());
    let p = ui.painter_at(rect);
    p.rect_filled(rect, CornerRadius::same(9), if resp.hovered() && !active { PANEL2 } else { fill });
    if active {
        let bar = egui::Rect::from_min_size(rect.left_top() + Vec2::new(0.0, 6.0), Vec2::new(3.0, rect.height() - 12.0));
        p.rect_filled(bar, CornerRadius::same(2), ACCENT);
    }
    p.text(rect.left_center() + Vec2::new(14.0, 0.0), egui::Align2::LEFT_CENTER, label, egui::FontId::proportional(14.0), text_col);
    if badge {
        p.circle_filled(rect.right_center() - Vec2::new(12.0, 0.0), 4.0, ACCENT);
    }
    resp.clicked()
}

fn text_button(label: &str, fill: Color32) -> egui::Button<'static> {
    egui::Button::new(RichText::new(label.to_string()).size(12.0).color(TEXT)).fill(fill).corner_radius(8)
}

fn primary_button(label: &str) -> egui::Button<'static> {
    egui::Button::new(RichText::new(label.to_string()).size(14.0).strong().color(Color32::from_rgb(8, 12, 16)))
        .fill(ACCENT).corner_radius(9).min_size(Vec2::new(0.0, 32.0))
}

fn danger_button(label: &str) -> egui::Button<'static> {
    egui::Button::new(RichText::new(label.to_string()).size(12.0).color(Color32::WHITE)).fill(BAD).corner_radius(8)
}

fn pack_chip(name: &str, accent: Color32, selected: bool) -> egui::Button<'static> {
    let fill = if selected { accent.linear_multiply(0.35) } else { PANEL2 };
    let stroke = if selected { Stroke::new(1.5_f32, accent) } else { Stroke::NONE };
    egui::Button::new(RichText::new(name.to_string()).size(12.5).color(if selected { TEXT } else { MUTED }))
        .fill(fill).stroke(stroke).corner_radius(14)
}

fn card_row<'a>(title: &'a str, sub: &'a str, selected: bool) -> impl egui::Widget + 'a {
    move |ui: &mut egui::Ui| {
        let fill = if selected { ACCENT.linear_multiply(0.18) } else { PANEL2 };
        egui::Frame::new().fill(fill).corner_radius(8).inner_margin(egui::Margin::symmetric(10, 7))
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.vertical(|ui| {
                    ui.label(RichText::new(title).size(13.0).color(if selected { ACCENT } else { TEXT }));
                    ui.label(RichText::new(sub).size(10.5).color(MUTED));
                });
            })
            .response
            .interact(Sense::click())
    }
}

fn chip(ui: &mut egui::Ui, label: &str, color: Color32) {
    egui::Frame::new().fill(color.linear_multiply(0.18)).corner_radius(14).inner_margin(egui::Margin::symmetric(12, 5))
        .show(ui, |ui| {
            ui.label(RichText::new(label).size(12.0).strong().color(color));
        });
}

fn step_pill(ui: &mut egui::Ui, n: usize, label: &str, done: bool) {
    let accent = if done { GOOD } else { ACCENT };
    ui.horizontal(|ui| {
        let (rect, _) = ui.allocate_exact_size(Vec2::splat(22.0), Sense::hover());
        let p = ui.painter_at(rect);
        p.circle_filled(rect.center(), 11.0, if done { GOOD.linear_multiply(0.3) } else { PANEL2 });
        p.circle_stroke(rect.center(), 11.0, Stroke::new(1.5_f32, accent));
        p.text(rect.center(), egui::Align2::CENTER_CENTER, n.to_string(), egui::FontId::proportional(12.0), accent);
        ui.label(RichText::new(label).size(12.5).color(if done { TEXT } else { MUTED }));
    });
}

fn step_sep(ui: &mut egui::Ui) {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(22.0, 22.0), Sense::hover());
    let p = ui.painter_at(rect);
    let y = rect.center().y;
    p.line_segment([egui::pos2(rect.left() + 2.0, y), egui::pos2(rect.right() - 2.0, y)], Stroke::new(1.5_f32, Color32::from_gray(70)));
}

fn lang_toggle(ui: &mut egui::Ui, lang: &mut Language) {
    for (l, txt) in [(Language::English, "EN"), (Language::Italian, "IT")] {
        let selected = *lang == l;
        let fill = if selected { ACCENT.linear_multiply(0.3) } else { PANEL2 };
        if ui.add(egui::Button::new(RichText::new(txt).size(12.0).color(if selected { ACCENT } else { MUTED })).fill(fill).corner_radius(7)).clicked() {
            *lang = l;
        }
    }
}

fn keypad_preview(ui: &mut egui::Ui, theme: Option<&crate::passthm::PasscodeTheme>) {
    let keys = [['1', '2', '3'], ['4', '5', '6'], ['7', '8', '9'], ['*', '0', '#']];
    let present: std::collections::HashSet<char> = theme.map(|t| t.buttons.iter().map(|b| b.key).collect()).unwrap_or_default();
    egui::Grid::new("keypad").spacing([10.0, 10.0]).show(ui, |ui| {
        for row in keys {
            for k in row {
                let has = present.contains(&k);
                let (rect, _) = ui.allocate_exact_size(Vec2::splat(48.0), Sense::hover());
                let p = ui.painter_at(rect);
                p.circle_filled(rect.center(), 22.0, if has { ACCENT.linear_multiply(0.4) } else { PANEL2 });
                p.text(rect.center(), egui::Align2::CENTER_CENTER, k.to_string(), egui::FontId::proportional(19.0), if has { TEXT } else { MUTED });
            }
            ui.end_row();
        }
    });
}

fn transport_label(app: &IcardApp, m: TransportMode) -> &'static str {
    match m {
        TransportMode::Auto => app.tr("Auto (USB)", "Auto (USB)"),
        TransportMode::UsbOnly => app.tr("USB only", "Solo USB"),
        TransportMode::WifiOnly => app.tr("WiFi only", "Solo WiFi"),
    }
}

fn keypad_label(k: KeypadLanguage) -> &'static str {
    match k {
        KeypadLanguage::Auto => "Auto (TelephonyUI-10)",
        KeypadLanguage::TelephonyUi10 => "TelephonyUI-10 (iOS 18+)",
        KeypadLanguage::TelephonyUi9 => "TelephonyUI-9 (iOS 16-17)",
        KeypadLanguage::TelephonyUi8 => "TelephonyUI-8 (Legacy)",
    }
}

const HELP_EN: &str = "\
1. Install 64-bit iTunes / Apple Mobile Device Support.
2. Connect your iPhone by USB and tap \"Trust This Computer\".
3. Click Scan, open Apple Wallet, tap the card, authenticate.
4. Pick a pack artwork or Upload your own image; drag and zoom to frame it.
5. Click Apply to iPhone, then force-close Wallet and reopen it.

Device writes need a trusted device and a license bound to this machine (HWID).
Open License... to activate, reset HWID, or manage licenses.";

const HELP_IT: &str = "\
1. Installa iTunes a 64 bit / Apple Mobile Device Support.
2. Collega l'iPhone via USB e tocca \"Autorizza questo computer\".
3. Premi Cerca, apri Apple Wallet, tocca la carta, autenticati.
4. Scegli una grafica o carica un'immagine; trascina e zooma per inquadrarla.
5. Premi Applica all'iPhone, poi chiudi e riapri Wallet.

Le scritture richiedono un dispositivo autorizzato e una licenza associata a questo PC (HWID).
Apri Licenza... per attivare, reimpostare l'HWID o gestire le licenze.";
