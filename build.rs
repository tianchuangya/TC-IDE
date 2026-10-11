fn main() {
    println!("cargo:rerun-if-env-changed=CARGO_CFG_TARGET_OS");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("linux") {
        return;
    }
    for library in [
        "libcairo.so.2",
        "libpangocairo-1.0.so.0",
        "libpango-1.0.so.0",
        "libgobject-2.0.so.0",
        "libglib-2.0.so.0",
        "libfontconfig.so.1",
    ] {
        println!("cargo:rustc-link-arg=-Wl,-l:{library}");
    }
}
