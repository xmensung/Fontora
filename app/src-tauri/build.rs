fn main() {
    // 守卫：release 构建必须带 custom-protocol。
    //
    // Tauri 2 的 tauri crate 里 dev = !custom_protocol（见 tauri/build.rs），
    // 也就是说只加 --release 不加 --features custom-protocol，编出来的仍是 dev 模式：
    // 前端资源不嵌入，运行时去连 devUrl(http://localhost:1420)，双击得到的是
    // ERR_CONNECTION_REFUSED 页面。0.9.0 / 0.10.0 两个包就是这么废掉的。
    //
    // 与其等打包发出去被发现，不如在这里直接停住。
    let profile = std::env::var("PROFILE").unwrap_or_default();
    if profile == "release" && std::env::var("CARGO_FEATURE_CUSTOM_PROTOCOL").is_err() {
        panic!(
            "\n\n\
            ============================================================\n\
             release 构建缺少 --features custom-protocol\n\
            ------------------------------------------------------------\n\
             这样编出来的 exe 是 dev 模式，不会内嵌前端，运行时会去连\n\
             http://localhost:1420 并报「拒绝连接」。\n\n\
             正确命令：\n\
             \x20   cargo build --release --features custom-protocol\n\
             \x20   或 pnpm tauri build\n\
            ============================================================\n"
        );
    }

    tauri_build::build()
}
