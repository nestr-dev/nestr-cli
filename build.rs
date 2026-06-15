fn main() {
    // On Windows (MSVC) the main thread's default stack is 1 MB, vs 8 MB on
    // Linux/macOS. Parsing the large clap command tree (20+ groups with nested
    // subcommands) inside the tokio runtime overflows 1 MB in debug builds, so
    // `nestr.exe` aborts with a stack overflow before it can even report a usage
    // error. Reserve 8 MB for the binary to match the other platforms. Windows
    // MSVC only — a no-op everywhere else.
    if std::env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("msvc") {
        println!("cargo:rustc-link-arg-bins=/STACK:8388608");
    }
}
