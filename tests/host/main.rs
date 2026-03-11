mod base64_tests;
mod batch_tests;
#[cfg(any(feature = "pico2w", feature = "pico1w"))]
mod board_tests;
mod fixtures;
mod interfaces;
mod led_tests;
mod mqtt_tests;
mod portal_tests;
mod protocol_tests;
mod router_tests;
mod storage_tests;
mod system_tests;
mod transport_tests;
mod ws_tests;
