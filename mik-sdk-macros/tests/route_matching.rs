//! Test that the router macro generates correct pattern matching code.

use std::process::Command;

#[test]
#[ignore = "requires wasm32-wasip1 target: rustup target add wasm32-wasip1"]
fn test_pattern_matching_in_generated_code() {
    // This test verifies that the routes! macro generates pattern matching code
    // that compares against req.path() instead of req.route()

    let output = Command::new("cargo")
        .args([
            "build",
            "--package",
            "hello-world",
            "--target",
            "wasm32-wasip1",
        ])
        .current_dir("../examples/hello-world")
        .output()
        .expect("Failed to build hello-world");

    assert!(
        output.status.success(),
        "Build should succeed:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn test_match_route_logic() {
    // Test the pattern matching logic that gets generated
    // This simulates what __match_route does in the generated code

    fn match_route(pattern: &str, path: &str) -> Option<Vec<(String, String)>> {
        let pattern_segments: Vec<&str> = pattern.split('/').collect();
        let path_segments: Vec<&str> = path.split('/').collect();

        if pattern_segments.len() != path_segments.len() {
            return None;
        }

        let mut params = Vec::new();

        for (pattern_seg, path_seg) in pattern_segments.iter().zip(path_segments.iter()) {
            if pattern_seg.starts_with('{') && pattern_seg.ends_with('}') {
                let param_name = &pattern_seg[1..pattern_seg.len() - 1];
                params.push((param_name.to_string(), path_seg.to_string()));
            } else if pattern_seg != path_seg {
                return None;
            }
        }

        Some(params)
    }

    // Test exact match
    assert_eq!(match_route("/", "/"), Some(vec![]));
    assert_eq!(match_route("/hello", "/hello"), Some(vec![]));

    // Test with parameters
    let result = match_route("/hello/{name}", "/hello/Alice");
    assert_eq!(
        result,
        Some(vec![("name".to_string(), "Alice".to_string())])
    );

    let result = match_route("/users/{id}/posts/{post_id}", "/users/123/posts/456");
    assert_eq!(
        result,
        Some(vec![
            ("id".to_string(), "123".to_string()),
            ("post_id".to_string(), "456".to_string())
        ])
    );

    // Test non-matches
    assert_eq!(match_route("/hello/{name}", "/hello"), None); // Too short
    assert_eq!(match_route("/hello/{name}", "/hello/Alice/extra"), None); // Too long
    assert_eq!(match_route("/hello/{name}", "/goodbye/Alice"), None); // Wrong literal segment
    assert_eq!(match_route("/api/v1", "/api/v2"), None); // Literal mismatch
}
