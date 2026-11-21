// Embed the Drover icon in drover-native.exe (resource 1, which GPUI's Windows backend loads for the window and
// taskbar). embed-resource does nothing when the target is not Windows.
fn main() {
    println!("cargo:rerun-if-changed=assets/drover.rc");
    println!("cargo:rerun-if-changed=assets/drover.ico");
    embed_resource::compile("assets/drover.rc", embed_resource::NONE).manifest_optional().expect("embed the Windows icon");
}
