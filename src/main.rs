#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

//! iCard — a desktop tool for customizing Apple Wallet card artwork and
//! passcode-keypad themes on a USB/WiFi connected iPhone.
//!
//! This is a from-scratch recreation of the original Windows "iCard" utility.
//! It reproduces the application: the egui UI, the device transport layer
//! (usbmux device enumeration, lockdown, AFC, syslog relay for hash capture),
//! the image pipeline, the saved-card database and the local activation gate.

mod activation;
mod app;
mod catalog;
mod i18n;
mod license;
mod model;
mod passthm;
mod saved_cards;

mod device;
mod wallet;

fn main() -> eframe::Result<()> {
    let icon = app::load_app_icon();

    let viewport = egui::ViewportBuilder::default()
        .with_title(format!("ProCard {}", env!("CARGO_PKG_VERSION")))
        .with_inner_size([1180.0, 760.0])
        .with_min_inner_size([960.0, 640.0])
        .with_icon(icon);

    let options = eframe::NativeOptions {
        viewport,
        ..Default::default()
    };

    eframe::run_native(
        "ProCard",
        options,
        Box::new(|cc| {
            egui_extras::install_image_loaders(&cc.egui_ctx);
            app::IcardApp::configure_fonts(&cc.egui_ctx);
            Ok(Box::new(app::IcardApp::new(cc)))
        }),
    )
}
