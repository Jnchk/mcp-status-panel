mod art {
    include!("src/icon_art.rs");
}

fn main() {
    println!("cargo:rerun-if-changed=src/icon_art.rs");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }

    let out_dir = std::path::PathBuf::from(std::env::var("OUT_DIR").expect("OUT_DIR"));
    let icon_path = out_dir.join("status-panel.ico");
    write_icon(&icon_path);

    let mut resource = winresource::WindowsResource::new();
    resource.set_icon(icon_path.to_str().expect("icon path"));
    resource.set("ProductName", "Status Panel");
    resource.set("FileDescription", "Status Panel");
    let version = std::env::var("CARGO_PKG_VERSION").unwrap_or_default();
    resource.set("FileVersion", &version);
    resource.set("ProductVersion", &version);
    resource.set_windres_path("x86_64-w64-mingw32-windres");
    resource.compile().expect("embed windows resources");
}

fn write_icon(path: &std::path::Path) {
    let mut directory = ico::IconDir::new(ico::ResourceType::Icon);
    for size in [16u32, 32, 48, 256] {
        let image = ico::IconImage::from_rgba_data(size, size, art::rgba(size));
        directory.add_entry(ico::IconDirEntry::encode(&image).expect("encode icon"));
    }
    let file = std::fs::File::create(path).expect("create ico");
    directory.write(file).expect("write ico");
}
