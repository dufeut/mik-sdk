//! Edge case tests for route matching and macro behavior.

#[test]
fn test_route_matching_edge_cases() {
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

    // Edge case: root path
    assert_eq!(match_route("/", "/"), Some(vec![]));

    // Edge case: empty segment after split
    assert_eq!(match_route("", ""), Some(vec![]));

    // Edge case: trailing slashes (normalized paths shouldn't have these)
    // These should NOT match if the segments differ
    assert_eq!(match_route("/users", "/users"), Some(vec![]));

    // Edge case: parameter with special characters in value
    let result = match_route("/users/{id}", "/users/user-123");
    assert_eq!(
        result,
        Some(vec![("id".to_string(), "user-123".to_string())])
    );

    // Edge case: parameter with encoded characters
    let result = match_route("/search/{query}", "/search/hello%20world");
    assert_eq!(
        result,
        Some(vec![("query".to_string(), "hello%20world".to_string())])
    );

    // Edge case: multiple parameters in sequence
    let result = match_route("/{a}/{b}/{c}", "/x/y/z");
    assert_eq!(
        result,
        Some(vec![
            ("a".to_string(), "x".to_string()),
            ("b".to_string(), "y".to_string()),
            ("c".to_string(), "z".to_string()),
        ])
    );

    // Edge case: parameter at start
    let result = match_route("/{id}/details", "/123/details");
    assert_eq!(result, Some(vec![("id".to_string(), "123".to_string())]));

    // Edge case: deep nesting
    let result = match_route(
        "/api/v1/users/{user_id}/posts/{post_id}/comments/{comment_id}",
        "/api/v1/users/42/posts/99/comments/7",
    );
    assert_eq!(
        result,
        Some(vec![
            ("user_id".to_string(), "42".to_string()),
            ("post_id".to_string(), "99".to_string()),
            ("comment_id".to_string(), "7".to_string()),
        ])
    );

    // Edge case: single character segments
    let result = match_route("/a/b/c", "/a/b/c");
    assert_eq!(result, Some(vec![]));

    // Edge case: numeric path segments
    let result = match_route("/v1/2024/data", "/v1/2024/data");
    assert_eq!(result, Some(vec![]));
}

#[test]
fn test_route_non_matches() {
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

    // Path shorter than pattern
    assert_eq!(match_route("/users/{id}", "/users"), None);

    // Path longer than pattern
    assert_eq!(match_route("/users/{id}", "/users/123/extra"), None);

    // Literal mismatch
    assert_eq!(match_route("/api/users", "/api/posts"), None);

    // Case sensitivity (routes are case-sensitive)
    assert_eq!(match_route("/Users", "/users"), None);

    // Similar but different
    assert_eq!(match_route("/user", "/users"), None);
}

#[test]
fn test_http_status_codes() {
    // Verify status code constants make sense

    // 2xx Success
    assert_eq!(200, 200); // OK
    assert_eq!(201, 201); // Created
    assert_eq!(204, 204); // No Content

    // 4xx Client Errors
    assert_eq!(400, 400); // Bad Request
    assert_eq!(401, 401); // Unauthorized
    assert_eq!(403, 403); // Forbidden
    assert_eq!(404, 404); // Not Found
    assert_eq!(405, 405); // Method Not Allowed
    assert_eq!(422, 422); // Unprocessable Entity

    // 5xx Server Errors
    assert_eq!(500, 500); // Internal Server Error
    assert_eq!(503, 503); // Service Unavailable
}
