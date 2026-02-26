fn main() {
    #[cfg(target_os = "windows")]
    {
        embed_resource::compile("../../assets/icon.ico", embed_resource::NONE);
    }
}
