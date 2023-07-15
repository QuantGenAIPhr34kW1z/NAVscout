fn main() {
    // Link to TFLite C API if feature enabled (cargo does not expose features to build.rs directly,
    // so we link unconditionally and rely on the library existing in the target image when used).
    // If you prefer strictness, guard this with env vars in your build pipeline.
    println!("cargo:rustc-link-lib=tensorflowlite_c");

    // Coral delegate is loaded at runtime via dlopen-like approach (no link required here).
}
