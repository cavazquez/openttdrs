#![no_main]

use std::io::Cursor;

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let _ = openttdrs_net::read_message(&mut Cursor::new(data));
});
