//! English / Spanish UI strings. iCard keeps the two languages side by side and
//! switches at runtime from the Activate/Language control.
#![allow(dead_code)] // the string table carries a few entries not yet surfaced

use crate::model::Language;

/// A flat table of every user-facing string, resolved for the active language.
pub struct Strings {
    pub lang: Language,
}

impl Strings {
    pub fn new(lang: Language) -> Self {
        Strings { lang }
    }

    fn pick(&self, en: &'static str, es: &'static str) -> &'static str {
        match self.lang {
            Language::Spanish => es,
            // Italian strings are provided by the studio UI's `tr` helper; legacy
            // strings fall back to English for Italian.
            Language::English | Language::Italian => en,
        }
    }

    // --- top bar / tabs ---
    pub fn tab_wallet(&self) -> &'static str {
        self.pick("Wallet", "Cartera")
    }
    pub fn tab_passcode(&self) -> &'static str {
        self.pick("Passcode", "Código")
    }
    pub fn tab_logs(&self) -> &'static str {
        self.pick("Logs", "Registros")
    }
    pub fn tab_logs_unread(&self) -> &'static str {
        self.pick("Logs [x]", "Registros [x]")
    }
    pub fn transport_auto(&self) -> &'static str {
        self.pick("Auto (USB preferred)", "Automático (USB preferido)")
    }
    pub fn transport_usb(&self) -> &'static str {
        self.pick("USB only", "Solo USB")
    }
    pub fn transport_wifi(&self) -> &'static str {
        self.pick("WiFi only", "Solo WiFi")
    }
    pub fn no_device(&self) -> &'static str {
        self.pick("No device", "Sin dispositivo")
    }
    pub fn activate(&self) -> &'static str {
        self.pick("Activate", "Activar")
    }
    pub fn activated(&self) -> &'static str {
        self.pick("Activated", "Activado")
    }
    pub fn activation_code(&self) -> &'static str {
        self.pick("Activation code", "Código de activación")
    }

    // --- wallet tab ---
    pub fn target_card_hash(&self) -> &'static str {
        self.pick("Target Card Hash", "Hash de la tarjeta")
    }
    pub fn pass_hash_hint(&self) -> &'static str {
        self.pick("Base64 pass hash…", "Hash base64 del pase…")
    }
    pub fn scan(&self) -> &'static str {
        self.pick("Scan", "Escanear")
    }
    pub fn stop(&self) -> &'static str {
        self.pick("Stop", "Detener")
    }
    pub fn scanning_syslog(&self) -> &'static str {
        self.pick(
            "Scanning syslog… Open Wallet on iPhone and tap your card",
            "Escaneando syslog… Abre Cartera en el iPhone y toca tu tarjeta",
        )
    }
    pub fn saved_cards(&self) -> &'static str {
        self.pick("Saved cards", "Tarjetas guardadas")
    }
    pub fn card_skin_artwork(&self) -> &'static str {
        self.pick("Card Skin Artwork", "Imagen de la tarjeta")
    }
    pub fn artwork_formats(&self) -> &'static str {
        self.pick(
            "PNG, JPG, WebP — auto-scaled to 1536x969",
            "PNG, JPG, WebP — escalado automático a 1536x969",
        )
    }
    pub fn drag_to_reposition(&self) -> &'static str {
        self.pick(
            "Drag inside the preview to reposition the crop.",
            "Arrastra dentro de la vista previa para reposicionar el recorte.",
        )
    }
    pub fn choose_image(&self) -> &'static str {
        self.pick("Choose image…", "Elegir imagen…")
    }
    pub fn no_artwork_loaded(&self) -> &'static str {
        self.pick("No artwork loaded", "Sin imagen")
    }
    pub fn wallet_preview(&self) -> &'static str {
        self.pick("Wallet Preview", "Vista previa de Cartera")
    }
    pub fn pass_canvas(&self) -> &'static str {
        self.pick("1536 x 969 px pass canvas", "Lienzo del pase 1536 x 969 px")
    }
    pub fn apply_card_skin(&self) -> &'static str {
        self.pick("Apply Card Skin", "Aplicar imagen")
    }
    pub fn write_to_iphone(&self) -> &'static str {
        self.pick("Write to iPhone", "Escribir en iPhone")
    }
    pub fn restore_original(&self) -> &'static str {
        self.pick("Restore Original", "Restaurar original")
    }
    pub fn after_apply_force_close(&self) -> &'static str {
        self.pick(
            "After applying, force close Apple Wallet and reopen it.",
            "Después de aplicar, fuerza el cierre de Apple Wallet y vuelve a abrirla.",
        )
    }

    // --- passcode tab ---
    pub fn passcode_theme(&self) -> &'static str {
        self.pick("Passcode Theme", "Tema del teclado")
    }
    pub fn passcode_subtitle(&self) -> &'static str {
        self.pick(
            "Custom lockscreen keypad from Cowabunga or Nugget",
            "Teclado de bloqueo personalizado de Cowabunga o Nugget",
        )
    }
    pub fn theme_package(&self) -> &'static str {
        self.pick("Theme Package", "Paquete de tema")
    }
    pub fn choose_passthm(&self) -> &'static str {
        self.pick("Choose .passthm…", "Elegir .passthm…")
    }
    pub fn passthm_hint(&self) -> &'static str {
        self.pick(
            "Choose a .passthm archive containing dialer artwork",
            "Elige un archivo .passthm con la imagen del teclado",
        )
    }
    pub fn keypad_language(&self) -> &'static str {
        self.pick("Keypad language", "Idioma del teclado")
    }
    pub fn generate_bold(&self) -> &'static str {
        self.pick(
            "Generate *-bold.png for devices with Bold Text turned ON (Settings → Display)",
            "Generar *-bold.png para dispositivos con Texto en negrita activado (Ajustes → Pantalla)",
        )
    }
    pub fn apply_passcode_theme(&self) -> &'static str {
        self.pick("Apply Passcode Theme", "Aplicar tema del teclado")
    }
    pub fn keypad_preview(&self) -> &'static str {
        self.pick("Keypad Preview", "Vista previa del teclado")
    }
    pub fn dialer_button_artwork(&self) -> &'static str {
        self.pick("Dialer button artwork", "Imagen de los botones")
    }
    pub fn after_apply_lock(&self) -> &'static str {
        self.pick(
            "After applying, lock your iPhone to see the new keypad.",
            "Después de aplicar, bloquea tu iPhone para ver el nuevo teclado.",
        )
    }

    // --- logs tab ---
    pub fn logs(&self) -> &'static str {
        self.pick("Logs", "Registros")
    }
    pub fn no_events(&self) -> &'static str {
        self.pick("No events logged yet.", "Aún no hay eventos registrados.")
    }
    pub fn copy_logs(&self) -> &'static str {
        self.pick("Copy Logs", "Copiar registros")
    }
    pub fn save_to_file(&self) -> &'static str {
        self.pick("Save to File…", "Guardar en archivo…")
    }
    pub fn clear(&self) -> &'static str {
        self.pick("Clear", "Limpiar")
    }

    // --- status / messages ---
    pub fn ready(&self) -> &'static str {
        self.pick(
            "Ready. Connect iPhone via USB or paired WiFi and unlock it.",
            "Listo. Conecta el iPhone por USB o WiFi emparejado y desbloquéalo.",
        )
    }
    pub fn need_activation(&self) -> &'static str {
        self.pick(
            "Action blocked: activation code required (local gate). Enter a valid activation code first.",
            "Acción bloqueada: se requiere código de activación (bloqueo local). Introduce un código válido primero.",
        )
    }
    pub fn invalid_activation(&self) -> &'static str {
        self.pick("Invalid activation code.", "Código de activación inválido.")
    }
    pub fn activation_ok(&self) -> &'static str {
        self.pick(
            "Activation successful (local gate).",
            "Activación exitosa (bloqueo local).",
        )
    }
    pub fn choose_image_first(&self) -> &'static str {
        self.pick(
            "Please choose a card skin image first.",
            "Por favor elige una imagen primero.",
        )
    }
    pub fn enter_hash_first(&self) -> &'static str {
        self.pick(
            "Please enter or scan a target card hash.",
            "Introduce o escanea un hash de tarjeta primero.",
        )
    }
    pub fn choose_theme_first(&self) -> &'static str {
        self.pick(
            "Please select a .passthm theme file first.",
            "Selecciona primero un archivo de tema .passthm.",
        )
    }
    pub fn select_device(&self) -> &'static str {
        self.pick(
            "Please select a connected iPhone.",
            "Selecciona un iPhone conectado.",
        )
    }

    // --- setup guide ---
    pub fn guide_title(&self) -> &'static str {
        self.pick("Setup & Card Hash Guide", "Guía de configuración y hash")
    }
    pub fn guide_subtitle(&self) -> &'static str {
        self.pick(
            "Everything you need to connect and capture your card",
            "Todo lo que necesitas para conectar y capturar tu tarjeta",
        )
    }
}
