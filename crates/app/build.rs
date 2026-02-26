fn main() {
    #[cfg(target_os = "windows")]
    {
        println!("cargo:rerun-if-changed=icon.rc");
        println!("cargo:rerun-if-changed=icon.debug.rc");
        println!("cargo:rerun-if-changed=app.manifest");
        println!("cargo:rerun-if-changed=app.debug.manifest");

        // 在 release 构建时使用 requireAdministrator 清单，debug 时用 asInvoker
        // 这样 cargo run 和 cargo test 可以直接工作，不会被 UAC 拦截
        let is_release = std::env::var("PROFILE").unwrap_or_default() == "release";
        let rc_file = if is_release {
            "icon.rc"
        } else {
            "icon.debug.rc"
        };
        embed_resource::compile(rc_file, embed_resource::NONE);
    }
}
