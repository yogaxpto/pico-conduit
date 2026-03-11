use std::{env, path::PathBuf};

fn main() {
    let out = PathBuf::from(env::var("OUT_DIR").unwrap());

    let is_embedded = env::var("CARGO_FEATURE_EMBEDDED").is_ok();
    let is_pico2w = env::var("CARGO_FEATURE_PICO2W").is_ok();
    let is_pico1w = env::var("CARGO_FEATURE_PICO1W").is_ok();

    if is_embedded {
        assert!(
            !(is_pico2w && is_pico1w),
            "features `pico2w` and `pico1w` are mutually exclusive"
        );
        assert!(
            is_pico2w || is_pico1w,
            "embedded builds require exactly one of `pico2w` or `pico1w`"
        );

        let src = if is_pico2w {
            "memory-pico2w.x"
        } else {
            "memory-pico1w.x"
        };
        std::fs::copy(src, out.join("memory.x")).unwrap();
        println!("cargo:rustc-link-search={}", out.display());
    }

    // Rebuild if these files change
    println!("cargo:rerun-if-changed=memory-pico2w.x");
    println!("cargo:rerun-if-changed=memory-pico1w.x");
    println!("cargo:rerun-if-changed=cyw43-firmware/43439A0.bin");
    println!("cargo:rerun-if-changed=cyw43-firmware/43439A0_clm.bin");

    // Rebuild if credential env vars change
    println!("cargo:rerun-if-env-changed=PICO_WIFI_SSID");
    println!("cargo:rerun-if-env-changed=PICO_WIFI_PASS");
}
