#![no_main]

use libfuzzer_sys::fuzz_target;
use mik_sdk::json;

fuzz_target!(|data: &[u8]| {
    // Test lazy parsing - should never panic
    if let Some(value) = json::try_parse(data) {
        // Exercise lazy path extraction
        let _ = value.path_str(&["a"]);
        let _ = value.path_int(&["b"]);
        let _ = value.path_float(&["c"]);
        let _ = value.path_bool(&["d"]);
        let _ = value.path_exists(&["e"]);
        let _ = value.path_is_null(&["f"]);

        // Exercise nested paths
        let _ = value.path_str(&["a", "b", "c"]);
        let _ = value.path_int(&["x", "y", "z"]);

        // Exercise tree operations (triggers full parse)
        let _ = value.get("key");
        let _ = value.at(0);
        let _ = value.keys();
        let _ = value.len();
        let _ = value.is_null();
        let _ = value.is_empty();

        // Exercise serialization
        let _ = value.to_string();
    }

    // Test eager parsing
    if let Some(value) = json::try_parse_full(data) {
        let _ = value.get("test");
        let _ = value.to_string();
    }
});
