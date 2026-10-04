fn main() {
    // The deb and the AppImage carry the ONNX Runtime library among the app's resources,
    // /usr/lib/Boltay/ort next to /usr/bin. A folder of its own: at the top of the resources
    // it would land in target/release over the link ort makes there to the same file.
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("linux") {
        println!("cargo:rustc-link-arg-bins=-Wl,-rpath,$ORIGIN/../lib/Boltay/ort");
    }
    tauri_build::build()
}
