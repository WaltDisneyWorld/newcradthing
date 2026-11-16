//! egui rendering for each part of the iCard window. Split out of `app.rs` to
//! keep the state machine and the view separate.

use egui::{Color32, RichText, Sense, Vec2};

use super::{IcardApp, Tab};
use crate::model::{KeypadLanguage, Language, TransportMode};

impl IcardApp {
    pub(super) fn top_bar(&mut self, ctx: &egui::Context) {
        egui::TopBottomPanel::top("top").show(ctx, |ui| {
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                let s = self.strings();
                // Logo + title.
                ui.label(RichText::new("iCard").heading().strong().color(Color32::from_rgb(60, 180, 200)));
                ui.separator();

                // Tabs.
                if ui.selectable_label(self.tab == Tab::Wallet, s.tab_wallet()).clicked() {
                    self.tab = Tab::Wallet;
                }
                if ui.selectable_label(self.tab == Tab::Passcode, s.tab_passcode()).clicked() {
                    self.tab = Tab::Passcode;
                }
                let logs_label = if self.logs_unread {
                    s.tab_logs_unread()
                } else {
                    s.tab_logs()
                };
                if ui.selectable_label(self.tab == Tab::Logs, logs_label).clicked() {
                    self.tab = Tab::Logs;
                    self.logs_unread = false;
                }

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    // Activation status / button.
                    if self.activation.is_active() {
                        ui.label(RichText::new(self.strings().activated()).color(Color32::from_rgb(90, 190, 110)));
                    } else if ui.button(self.strings().activate()).clicked() {
                        self.show_activation = true;
                    }
                    ui.separator();

                    // Language toggle.
                    let mut lang = self.settings.language;
                    egui::ComboBox::from_id_salt("lang")
                        .selected_text(match lang {
                            Language::English => "EN",
                            Language::Spanish => "ES",
                        })
                        .show_ui(ui, |ui| {
                            ui.selectable_value(&mut lang, Language::English, "English");
                            ui.selectable_value(&mut lang, Language::Spanish, "Español");
                        });
                    if lang != self.settings.language {
                        self.settings.language = lang;
                        crate::saved_cards::save_settings(&self.settings);
                        self.log("Language changed.");
                    }
                    ui.separator();

                    // Device picker + refresh.
                    if ui.button("⟳").on_hover_text("Rescan devices").clicked() {
                        self.refresh_devices();
                    }
                    let current = self
                        .selected_device()
                        .map(|d| d.label())
                        .unwrap_or_else(|| self.strings().no_device().to_string());
                    let mut sel = self.selected_device;
                    egui::ComboBox::from_id_salt("device")
                        .width(220.0)
                        .selected_text(current)
                        .show_ui(ui, |ui| {
                            if self.devices.is_empty() {
                                ui.label(self.strings().no_device());
                            }
                            for (i, d) in self.devices.iter().enumerate() {
                                ui.selectable_value(&mut sel, i, d.label());
                            }
                        });
                    self.selected_device = sel;
                    ui.separator();

                    // Transport.
                    let mut mode = self.settings.transport;
                    egui::ComboBox::from_id_salt("transport")
                        .selected_text(transport_label(&self.strings(), mode))
                        .show_ui(ui, |ui| {
                            for m in TransportMode::all() {
                                ui.selectable_value(&mut mode, m, transport_label(&self.strings(), m));
                            }
                        });
                    if mode != self.settings.transport {
                        self.settings.transport = mode;
                        crate::saved_cards::save_settings(&self.settings);
                        self.refresh_devices();
                    }
                });
            });
            ui.add_space(4.0);
        });
    }

    pub(super) fn bottom_bar(&mut self, ctx: &egui::Context) {
        egui::TopBottomPanel::bottom("status").show(ctx, |ui| {
            ui.add_space(2.0);
            ui.horizontal(|ui| {
                if self.busy {
                    ui.spinner();
                }
                ui.label(RichText::new(&self.status).small());
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(
                        RichText::new(self.support.describe())
                            .small()
                            .color(if self.support.is_ready() {
                                Color32::from_rgb(120, 180, 130)
                            } else {
                                Color32::from_gray(150)
                            }),
                    );
                });
            });
            ui.add_space(2.0);
        });
    }

    pub(super) fn wallet_tab(&mut self, ui: &mut egui::Ui) {
        let s = self.strings();
        egui::CollapsingHeader::new(format!("❔ {}", s.guide_title()))
            .default_open(self.show_guide)
            .show(ui, |ui| self.setup_guide(ui));
        ui.separator();

        egui::ScrollArea::vertical().show(ui, |ui| {
            ui.columns(2, |cols| {
                self.wallet_left(&mut cols[0]);
                self.wallet_right(&mut cols[1]);
            });
        });
    }

    fn wallet_left(&mut self, ui: &mut egui::Ui) {
        let s = self.strings();
        ui.heading(s.target_card_hash());
        ui.horizontal(|ui| {
            ui.add(
                egui::TextEdit::singleline(&mut self.hash_input)
                    .hint_text(s.pass_hash_hint())
                    .desired_width(260.0),
            );
            if self.scanning {
                if ui.button(s.stop()).clicked() {
                    self.stop_scan();
                }
            } else if ui.button(s.scan()).clicked() {
                self.start_scan();
            }
        });
        if self.scanning {
            ui.horizontal(|ui| {
                ui.spinner();
                ui.label(RichText::new(s.scanning_syslog()).italics().small());
            });
        }

        ui.add_space(6.0);
        ui.label(RichText::new(s.saved_cards()).strong());
        egui::ScrollArea::vertical()
            .id_salt("saved")
            .max_height(120.0)
            .show(ui, |ui| {
                if self.saved.is_empty() {
                    ui.label(RichText::new("—").weak());
                }
                let mut pick = None;
                for c in &self.saved {
                    let label = if c.name.is_empty() {
                        crate::model::short_udid(&c.hash)
                    } else {
                        format!("{} — {}", c.name, crate::model::short_udid(&c.hash))
                    };
                    if ui.selectable_label(self.hash_input == c.hash, label).clicked() {
                        pick = Some(c.hash.clone());
                    }
                }
                if let Some(h) = pick {
                    self.hash_input = h;
                }
            });

        ui.add_space(8.0);
        ui.heading(s.card_skin_artwork());
        ui.label(RichText::new(s.artwork_formats()).small().weak());
        ui.horizontal(|ui| {
            if ui.button(s.choose_image()).clicked() {
                if let Some(path) = rfd::FileDialog::new()
                    .add_filter("Images", &["png", "jpg", "jpeg", "webp"])
                    .pick_file()
                {
                    self.load_image(path);
                }
            }
            let name = if self.source_name.is_empty() {
                s.no_artwork_loaded().to_string()
            } else {
                self.source_name.clone()
            };
            ui.label(RichText::new(name).small());
        });
        ui.label(RichText::new(s.drag_to_reposition()).small().weak());

        ui.add_space(10.0);
        let can_restore = airlift::restore_available(&self.hash_input);
        ui.horizontal(|ui| {
            let apply = egui::Button::new(RichText::new(format!("⬆ {}", s.write_to_iphone())).strong());
            if ui.add_enabled(!self.busy, apply).clicked() {
                self.apply_card_skin();
            }
            let restore = egui::Button::new(s.restore_original());
            if ui.add_enabled(!self.busy && can_restore, restore).clicked() {
                self.restore_original();
            }
        });
        if !can_restore {
            ui.label(
                RichText::new("Need: Restore Original — apply a card skin once to create an original backup.")
                    .small()
                    .weak(),
            );
        }
        ui.label(RichText::new(s.after_apply_force_close()).small().weak());
    }

    fn wallet_right(&mut self, ui: &mut egui::Ui) {
        let s = self.strings();
        ui.heading(s.wallet_preview());
        ui.label(RichText::new(s.pass_canvas()).small().weak());

        let avail = ui.available_width().min(560.0).max(240.0);
        let size = Vec2::new(avail, avail * (969.0_f32 / 1536.0));
        let (rect, resp) = ui.allocate_exact_size(size, Sense::drag());
        let painter = ui.painter_at(rect);
        painter.rect_filled(rect, egui::CornerRadius::same(8), Color32::from_gray(28));
        if let Some(tex) = &self.preview_tex {
            let uv = egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0));
            painter.image(tex.id(), rect, uv, Color32::WHITE);
        } else {
            painter.text(
                rect.center(),
                egui::Align2::CENTER_CENTER,
                s.no_artwork_loaded(),
                egui::FontId::proportional(16.0),
                Color32::from_gray(140),
            );
        }
        painter.rect_stroke(
            rect,
            egui::CornerRadius::same(8),
            egui::Stroke::new(1.0_f32, Color32::from_gray(70)),
            egui::StrokeKind::Inside,
        );

        if resp.dragged() && self.work.is_some() {
            let d = resp.drag_delta();
            self.crop_offset.x = (self.crop_offset.x - d.x / rect.width()).clamp(0.0, 1.0);
            self.crop_offset.y = (self.crop_offset.y - d.y / rect.height()).clamp(0.0, 1.0);
            self.preview_dirty = true;
        }
    }

    pub(super) fn passcode_tab(&mut self, ui: &mut egui::Ui) {
        let s = self.strings();
        ui.heading(s.passcode_theme());
        ui.label(RichText::new(s.passcode_subtitle()).small().weak());
        ui.separator();

        ui.columns(2, |cols| {
            let ui = &mut cols[0];
            ui.label(RichText::new(s.theme_package()).strong());
            ui.label(RichText::new(s.passthm_hint()).small().weak());
            ui.horizontal(|ui| {
                if ui.button(s.choose_passthm()).clicked() {
                    if let Some(path) = rfd::FileDialog::new()
                        .add_filter("Passcode theme", &["passthm", "zip"])
                        .pick_file()
                    {
                        match crate::passthm::load(&path) {
                            Ok(t) => {
                                self.log(format!(
                                    "Passcode theme loaded: '{}' ({} buttons)",
                                    t.name,
                                    t.button_count()
                                ));
                                self.theme = Some(t);
                            }
                            Err(e) => {
                                self.log(format!("Failed to load theme: {e}"));
                                self.status = e.to_string();
                            }
                        }
                    }
                }
                let label = self
                    .theme
                    .as_ref()
                    .map(|t| format!("{} ({})", t.name, t.button_count()))
                    .unwrap_or_else(|| "—".into());
                ui.label(RichText::new(label).small());
            });

            ui.add_space(8.0);
            ui.label(RichText::new(s.keypad_language()).strong());
            let mut lang = self.settings.keypad_language;
            egui::ComboBox::from_id_salt("keypad_lang")
                .selected_text(keypad_label(lang))
                .show_ui(ui, |ui| {
                    for k in KeypadLanguage::all() {
                        ui.selectable_value(&mut lang, k, keypad_label(k));
                    }
                });
            if lang != self.settings.keypad_language {
                self.settings.keypad_language = lang;
                crate::saved_cards::save_settings(&self.settings);
            }

            let mut bold = self.settings.generate_bold_variant;
            if ui.checkbox(&mut bold, s.generate_bold()).changed() {
                self.settings.generate_bold_variant = bold;
                crate::saved_cards::save_settings(&self.settings);
            }

            ui.add_space(10.0);
            let apply = egui::Button::new(RichText::new(format!("⬆ {}", s.apply_passcode_theme())).strong());
            if ui.add_enabled(!self.busy && self.theme.is_some(), apply).clicked() {
                self.apply_theme();
            }
            ui.label(RichText::new(s.after_apply_lock()).small().weak());

            let ui = &mut cols[1];
            ui.heading(s.keypad_preview());
            ui.label(RichText::new(s.dialer_button_artwork()).small().weak());
            keypad_preview(ui, self.theme.as_ref());
        });
    }

    pub(super) fn logs_tab(&mut self, ui: &mut egui::Ui) {
        let s = self.strings();
        ui.horizontal(|ui| {
            if ui.button(s.copy_logs()).clicked() {
                ui.ctx().copy_text(self.logs.join("\n"));
            }
            if ui.button(s.save_to_file()).clicked() {
                if let Some(path) = rfd::FileDialog::new()
                    .set_file_name("icard-diagnostics.log")
                    .add_filter("Log", &["log", "txt"])
                    .save_file()
                {
                    let _ = std::fs::write(path, self.logs.join("\n"));
                }
            }
            if ui.button(s.clear()).clicked() {
                self.logs.clear();
            }
        });
        ui.separator();
        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .stick_to_bottom(true)
            .show(ui, |ui| {
                if self.logs.is_empty() {
                    ui.label(RichText::new(s.no_events()).weak());
                }
                for line in &self.logs {
                    ui.label(RichText::new(line).monospace().small());
                }
            });
    }

    fn setup_guide(&mut self, ui: &mut egui::Ui) {
        let s = self.strings();
        ui.label(RichText::new(s.guide_subtitle()).italics());
        ui.add_space(4.0);
        let body = match self.settings.language {
            Language::English => GUIDE_EN,
            Language::Spanish => GUIDE_ES,
        };
        ui.label(body);
    }

    pub(super) fn activation_window(&mut self, ctx: &egui::Context) {
        if !self.show_activation {
            return;
        }
        let mut open = true;
        egui::Window::new(self.strings().activate())
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .open(&mut open)
            .show(ctx, |ui| {
                let s = self.strings();
                ui.label(s.activation_code());
                ui.add(
                    egui::TextEdit::singleline(&mut self.activation_input)
                        .hint_text("XXXX-XXXX-XXXX-XXXX")
                        .desired_width(240.0),
                );
                ui.horizontal(|ui| {
                    if ui.button(s.activate()).clicked() {
                        let code = self.activation_input.clone();
                        if self.activation.try_activate(&code) {
                            self.activation_msg = self.strings().activation_ok().to_string();
                            self.log(self.strings().activation_ok());
                            self.show_activation = false;
                        } else {
                            self.activation_msg = self.strings().invalid_activation().to_string();
                        }
                    }
                });
                if !self.activation_msg.is_empty() {
                    let ok = self.activation.is_active();
                    ui.label(RichText::new(&self.activation_msg).color(if ok {
                        Color32::from_rgb(90, 190, 110)
                    } else {
                        Color32::from_rgb(210, 110, 110)
                    }));
                }
            });
        if !open {
            self.show_activation = false;
        }
    }
}

// Pull the i18n import into scope.
use crate::i18n::Strings;
use crate::wallet::airlift;

fn transport_label(s: &Strings, m: TransportMode) -> &'static str {
    match m {
        TransportMode::Auto => s.transport_auto(),
        TransportMode::UsbOnly => s.transport_usb(),
        TransportMode::WifiOnly => s.transport_wifi(),
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

/// Simple dial-pad mock showing which buttons the loaded theme provides.
fn keypad_preview(ui: &mut egui::Ui, theme: Option<&crate::passthm::PasscodeTheme>) {
    let keys = [
        ['1', '2', '3'],
        ['4', '5', '6'],
        ['7', '8', '9'],
        ['*', '0', '#'],
    ];
    let present: std::collections::HashSet<char> = theme
        .map(|t| t.buttons.iter().map(|b| b.key).collect())
        .unwrap_or_default();
    egui::Grid::new("keypad").spacing([8.0, 8.0]).show(ui, |ui| {
        for row in keys {
            for k in row {
                let has = present.contains(&k);
                let (rect, _) = ui.allocate_exact_size(Vec2::splat(52.0), Sense::hover());
                let p = ui.painter_at(rect);
                p.circle_filled(
                    rect.center(),
                    24.0,
                    if has { Color32::from_rgb(55, 90, 120) } else { Color32::from_gray(45) },
                );
                p.text(
                    rect.center(),
                    egui::Align2::CENTER_CENTER,
                    k.to_string(),
                    egui::FontId::proportional(20.0),
                    Color32::WHITE,
                );
            }
            ui.end_row();
        }
    });
}

const GUIDE_EN: &str = "\
Prerequisites
- 64-bit iTunes or Apple Mobile Device Support installed
- First-time setup: connect by USB and tap \"Trust this Computer\"
- WiFi: enable WiFi sync, then use the same local network
- Select Auto, USB only, or WiFi only in the top bar

Finding Your Card Hash
1. Click \"Scan\" in the Wallet tab
2. Open Apple Wallet on your iPhone
3. Tap the card you want to customize
4. iCard captures the pass hash automatically
5. Click \"Stop\" once detected

Applying a Card Skin
1. Choose an image, drag to reposition the crop
2. Click \"Write to iPhone\"
3. Force close Apple Wallet and reopen it to see the new image";

const GUIDE_ES: &str = "\
Requisitos previos
- iTunes de 64 bits o Apple Mobile Device Support instalado
- Primera vez: conecta por USB y toca \"Confiar en este equipo\"
- WiFi: activa la sincronización WiFi y usa la misma red local
- Elige Automático, Solo USB o Solo WiFi en la barra superior

Cómo encontrar el hash de tu tarjeta
1. Haz clic en \"Escanear\" en la pestaña Cartera
2. Abre Apple Wallet en tu iPhone
3. Toca la tarjeta que quieres personalizar
4. iCard captura el hash del pase automáticamente
5. Haz clic en \"Detener\" cuando se detecte

Aplicar la imagen
1. Elige una imagen y arrastra para reposicionar el recorte
2. Haz clic en \"Escribir en iPhone\"
3. Fuerza el cierre de Apple Wallet y vuelve a abrirla";
