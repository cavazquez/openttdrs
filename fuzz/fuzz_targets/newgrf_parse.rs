#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let _ = openttdrs_core::newgrf_config::parse_grf_full(data);
});
