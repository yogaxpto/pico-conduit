use std::{env, path::PathBuf};

fn main() {
    let out = PathBuf::from(env::var("OUT_DIR").unwrap());
    // Copy memory.x to OUT_DIR so the linker can find it
    std::fs::copy("memory.x", out.join("memory.x")).unwrap();
    println!("cargo:rustc-link-search={}", out.display());

    // Rebuild if these files change
    println!("cargo:rerun-if-changed=memory.x");
    println!("cargo:rerun-if-changed=cyw43-firmware/43439A0.bin");
    println!("cargo:rerun-if-changed=cyw43-firmware/43439A0_clm.bin");

    // Rebuild if credential env vars change (Phase 6a compile-time override)
    println!("cargo:rerun-if-env-changed=PICO_WIFI_SSID");
    println!("cargo:rerun-if-env-changed=PICO_WIFI_PASS");
}
