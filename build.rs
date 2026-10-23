fn main() {
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        let mut res = winresource::WindowsResource::new();
        res.set_icon("src/icon.ico");
        res.set("ProductName", "ScreenPatch");
        res.set("FileDescription", "ScreenPatch");
        res.set("OriginalFilename", "ScreenPatch.exe");
        res.compile().unwrap();
    }
}