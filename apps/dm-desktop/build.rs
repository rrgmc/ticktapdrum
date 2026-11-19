//! Puts the app icon and its name into the Windows executable. Other systems take the icon from
//! their package (the macOS app bundle, the Linux desktop file).

fn main() {
    println!("cargo:rerun-if-changed=assets/icon.ico");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        let mut res = winresource::WindowsResource::new();
        res.set_icon("assets/icon.ico");
        res.set("ProductName", "TickTapDrum");
        res.set("FileDescription", "TickTapDrum");
        if let Err(e) = res.compile() {
            // A missing resource compiler costs only the icon, not the build.
            println!("cargo:warning=no Windows icon: {e}");
        }
    }
}
