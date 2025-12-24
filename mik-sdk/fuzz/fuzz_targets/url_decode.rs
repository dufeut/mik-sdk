#![no_main]

use libfuzzer_sys::fuzz_target;
use mik_sdk::url_decode;

fuzz_target!(|data: &str| {
    // url_decode should never panic, regardless of input
    let _ = url_decode(data);
});
