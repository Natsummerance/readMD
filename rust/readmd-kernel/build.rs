fn main() {
    #[cfg(target_os = "windows")]
    {
        let mut res = winres::WindowsResource::new();
        res.set_icon("../../assets/readmd.ico");
        if let Err(e) = res.compile() {
            eprintln!("cargo:warning=winres icon compile error: {}", e);
        }
    }
}
