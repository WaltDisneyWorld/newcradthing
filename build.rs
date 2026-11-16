fn main() {
    // On Windows, iCard ships as a GUI app (no console window) and carries an
    // application manifest for DPI awareness. Nothing is required on other
    // platforms, where the Apple device layer is a stub (see src/device/apple).
    #[cfg(windows)]
    {
        // Keep the build resilient even without a resource compiler.
        println!("cargo:rustc-link-arg-bins=/SUBSYSTEM:WINDOWS");
        println!("cargo:rustc-link-arg-bins=/ENTRY:mainCRTStartup");
    }
}
