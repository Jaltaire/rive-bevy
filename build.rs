use std::env;

fn main() {
    println!("cargo::rustc-check-cfg=cfg(metal_renderer_native)");
    println!("cargo:rerun-if-env-changed=CARGO_FEATURE_METAL_RENDERER");

    let metal_renderer = env::var_os("CARGO_FEATURE_METAL_RENDERER").is_some();
    let target_os = env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();

    if metal_renderer && matches!(target_os.as_str(), "macos" | "ios") {
        println!("cargo::rustc-cfg=metal_renderer_native");
    }
}
