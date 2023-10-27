fn main() {
    // Only link TFLite when the feature is enabled
    #[cfg(feature = "vision-tflite")]
    {
        // Add common library search paths
        println!("cargo:rustc-link-search=native=/usr/lib");
        println!("cargo:rustc-link-search=native=/usr/local/lib");
        println!("cargo:rustc-link-search=native=/usr/lib/x86_64-linux-gnu");
        println!("cargo:rustc-link-search=native=/usr/lib/aarch64-linux-gnu");

        // Allow override via environment variable
        if let Ok(path) = std::env::var("TFLITE_LIB_DIR") {
            println!("cargo:rustc-link-search=native={}", path);
        }
    }
}
