//! Tests for the ids! macro.
//!
//! The ids! macro collects field values from a Vec of structs for batched loading.
//! Usage: `ids!(collection)` for default "id" field, or `ids!(collection, field_name)`
//! for a custom field.

use mik_sdk_macros::ids;

// ============================================================================
// Test Structs
// ============================================================================

/// Simple struct with String id
#[allow(dead_code)]
struct User {
    id: String,
    name: String,
}

/// Struct with i32 id
#[allow(dead_code)]
struct Post {
    id: i32,
    title: String,
    user_id: i32,
}

/// Struct with various field types
#[allow(dead_code)]
struct Order {
    id: u64,
    customer_id: String,
    product_code: String,
    quantity: u32,
}

/// Struct with Option field
#[allow(dead_code)]
struct Comment {
    id: i32,
    post_id: Option<i32>,
    content: String,
}

/// Struct with nested field (for testing clone)
#[allow(dead_code)]
struct Item {
    id: String,
    tags: Vec<String>,
}

// ============================================================================
// Basic Usage Tests
// ============================================================================

#[test]
fn test_ids_default_field_string() {
    let users = [
        User {
            id: "u1".to_string(),
            name: "Alice".to_string(),
        },
        User {
            id: "u2".to_string(),
            name: "Bob".to_string(),
        },
        User {
            id: "u3".to_string(),
            name: "Charlie".to_string(),
        },
    ];

    let user_ids: Vec<String> = ids!(users);

    assert_eq!(user_ids.len(), 3);
    assert_eq!(user_ids[0], "u1");
    assert_eq!(user_ids[1], "u2");
    assert_eq!(user_ids[2], "u3");
}

#[test]
fn test_ids_default_field_i32() {
    let posts = [
        Post {
            id: 1,
            title: "First".to_string(),
            user_id: 10,
        },
        Post {
            id: 2,
            title: "Second".to_string(),
            user_id: 20,
        },
        Post {
            id: 3,
            title: "Third".to_string(),
            user_id: 10,
        },
    ];

    let post_ids: Vec<i32> = ids!(posts);

    assert_eq!(post_ids, vec![1, 2, 3]);
}

#[test]
fn test_ids_default_field_u64() {
    let orders = [
        Order {
            id: 1000,
            customer_id: "c1".to_string(),
            product_code: "P001".to_string(),
            quantity: 5,
        },
        Order {
            id: 2000,
            customer_id: "c2".to_string(),
            product_code: "P002".to_string(),
            quantity: 3,
        },
    ];

    let order_ids: Vec<u64> = ids!(orders);

    assert_eq!(order_ids, vec![1000, 2000]);
}

// ============================================================================
// Custom Field Name Tests
// ============================================================================

#[test]
fn test_ids_custom_field_user_id() {
    let posts = [
        Post {
            id: 1,
            title: "Post 1".to_string(),
            user_id: 100,
        },
        Post {
            id: 2,
            title: "Post 2".to_string(),
            user_id: 200,
        },
        Post {
            id: 3,
            title: "Post 3".to_string(),
            user_id: 100,
        },
    ];

    let user_ids: Vec<i32> = ids!(posts, user_id);

    assert_eq!(user_ids, vec![100, 200, 100]);
}

#[test]
fn test_ids_custom_field_customer_id() {
    let orders = [
        Order {
            id: 1,
            customer_id: "cust_001".to_string(),
            product_code: "PROD_A".to_string(),
            quantity: 10,
        },
        Order {
            id: 2,
            customer_id: "cust_002".to_string(),
            product_code: "PROD_B".to_string(),
            quantity: 5,
        },
        Order {
            id: 3,
            customer_id: "cust_001".to_string(),
            product_code: "PROD_C".to_string(),
            quantity: 2,
        },
    ];

    let customer_ids: Vec<String> = ids!(orders, customer_id);

    assert_eq!(customer_ids.len(), 3);
    assert_eq!(customer_ids[0], "cust_001");
    assert_eq!(customer_ids[1], "cust_002");
    assert_eq!(customer_ids[2], "cust_001");
}

#[test]
fn test_ids_custom_field_product_code() {
    let orders = [
        Order {
            id: 1,
            customer_id: "c1".to_string(),
            product_code: "ABC".to_string(),
            quantity: 1,
        },
        Order {
            id: 2,
            customer_id: "c2".to_string(),
            product_code: "DEF".to_string(),
            quantity: 2,
        },
    ];

    let product_codes: Vec<String> = ids!(orders, product_code);

    assert_eq!(product_codes, vec!["ABC", "DEF"]);
}

#[test]
fn test_ids_custom_field_quantity() {
    let orders = [
        Order {
            id: 1,
            customer_id: "c1".to_string(),
            product_code: "P1".to_string(),
            quantity: 10,
        },
        Order {
            id: 2,
            customer_id: "c2".to_string(),
            product_code: "P2".to_string(),
            quantity: 20,
        },
        Order {
            id: 3,
            customer_id: "c3".to_string(),
            product_code: "P3".to_string(),
            quantity: 30,
        },
    ];

    let quantities: Vec<u32> = ids!(orders, quantity);

    assert_eq!(quantities, vec![10, 20, 30]);
}

// ============================================================================
// Empty Collection Tests
// ============================================================================

#[test]
fn test_ids_empty_collection_string() {
    let users: Vec<User> = vec![];

    let user_ids: Vec<String> = ids!(users);

    assert!(user_ids.is_empty());
}

#[test]
fn test_ids_empty_collection_i32() {
    let posts: Vec<Post> = vec![];

    let post_ids: Vec<i32> = ids!(posts);

    assert!(post_ids.is_empty());
}

#[test]
fn test_ids_empty_collection_custom_field() {
    let orders: Vec<Order> = vec![];

    let customer_ids: Vec<String> = ids!(orders, customer_id);

    assert!(customer_ids.is_empty());
}

// ============================================================================
// Single Element Tests
// ============================================================================

#[test]
fn test_ids_single_element_default_field() {
    let users = [User {
        id: "only_one".to_string(),
        name: "Solo".to_string(),
    }];

    let user_ids: Vec<String> = ids!(users);

    assert_eq!(user_ids.len(), 1);
    assert_eq!(user_ids[0], "only_one");
}

#[test]
fn test_ids_single_element_custom_field() {
    let posts = [Post {
        id: 42,
        title: "The Answer".to_string(),
        user_id: 999,
    }];

    let user_ids: Vec<i32> = ids!(posts, user_id);

    assert_eq!(user_ids.len(), 1);
    assert_eq!(user_ids[0], 999);
}

// ============================================================================
// Option Field Tests
// ============================================================================

#[test]
fn test_ids_option_field() {
    let comments = [
        Comment {
            id: 1,
            post_id: Some(100),
            content: "Great!".to_string(),
        },
        Comment {
            id: 2,
            post_id: None,
            content: "Orphan comment".to_string(),
        },
        Comment {
            id: 3,
            post_id: Some(200),
            content: "Nice!".to_string(),
        },
    ];

    let post_ids: Vec<Option<i32>> = ids!(comments, post_id);

    assert_eq!(post_ids.len(), 3);
    assert_eq!(post_ids[0], Some(100));
    assert_eq!(post_ids[1], None);
    assert_eq!(post_ids[2], Some(200));
}

// ============================================================================
// Complex Type Tests
// ============================================================================

#[test]
fn test_ids_vec_field() {
    let items = [
        Item {
            id: "i1".to_string(),
            tags: vec!["red".to_string(), "large".to_string()],
        },
        Item {
            id: "i2".to_string(),
            tags: vec!["blue".to_string()],
        },
    ];

    let all_tags: Vec<Vec<String>> = ids!(items, tags);

    assert_eq!(all_tags.len(), 2);
    assert_eq!(all_tags[0], vec!["red", "large"]);
    assert_eq!(all_tags[1], vec!["blue"]);
}

// ============================================================================
// Reference/Borrow Tests (verifies .clone() works correctly)
// ============================================================================

#[test]
fn test_ids_preserves_original() {
    let users = [
        User {
            id: "u1".to_string(),
            name: "Alice".to_string(),
        },
        User {
            id: "u2".to_string(),
            name: "Bob".to_string(),
        },
    ];

    // Extract IDs
    let user_ids: Vec<String> = ids!(users);

    // Original collection should still be usable
    assert_eq!(users.len(), 2);
    assert_eq!(users[0].id, "u1");
    assert_eq!(users[0].name, "Alice");

    // And extracted IDs should be correct
    assert_eq!(user_ids, vec!["u1", "u2"]);
}

// ============================================================================
// Batched Loading Pattern Tests (real-world usage)
// ============================================================================

#[test]
fn test_ids_for_batched_loading_pattern() {
    // Simulate fetching posts and then loading their users
    let posts = [
        Post {
            id: 1,
            title: "Rust Tips".to_string(),
            user_id: 10,
        },
        Post {
            id: 2,
            title: "WASM Guide".to_string(),
            user_id: 20,
        },
        Post {
            id: 3,
            title: "More Rust".to_string(),
            user_id: 10,
        },
    ];

    // Collect user_ids for batched query: WHERE user_id IN (...)
    let user_ids: Vec<i32> = ids!(posts, user_id);

    // Should get [10, 20, 10] - duplicates are preserved (let the DB handle dedup)
    assert_eq!(user_ids, vec![10, 20, 10]);
}

#[test]
fn test_ids_unique_pattern() {
    // If uniqueness is needed, users can dedup themselves
    let posts = [
        Post {
            id: 1,
            title: "A".to_string(),
            user_id: 10,
        },
        Post {
            id: 2,
            title: "B".to_string(),
            user_id: 20,
        },
        Post {
            id: 3,
            title: "C".to_string(),
            user_id: 10,
        },
        Post {
            id: 4,
            title: "D".to_string(),
            user_id: 30,
        },
        Post {
            id: 5,
            title: "E".to_string(),
            user_id: 20,
        },
    ];

    let user_ids: Vec<i32> = ids!(posts, user_id);

    // Raw extraction includes duplicates
    assert_eq!(user_ids, vec![10, 20, 10, 30, 20]);

    // User can dedup if needed
    let mut unique_ids = user_ids;
    unique_ids.sort_unstable();
    unique_ids.dedup();
    assert_eq!(unique_ids, vec![10, 20, 30]);
}

// ============================================================================
// Expression as Collection Tests
// ============================================================================

#[test]
fn test_ids_with_method_call_expression() {
    fn get_users() -> Vec<User> {
        vec![
            User {
                id: "u1".to_string(),
                name: "Alice".to_string(),
            },
            User {
                id: "u2".to_string(),
                name: "Bob".to_string(),
            },
        ]
    }

    let users = get_users();
    let user_ids: Vec<String> = ids!(users);

    assert_eq!(user_ids, vec!["u1", "u2"]);
}

#[test]
fn test_ids_with_reference() {
    let users = vec![
        User {
            id: "u1".to_string(),
            name: "Alice".to_string(),
        },
        User {
            id: "u2".to_string(),
            name: "Bob".to_string(),
        },
    ];

    // Can use reference to collection
    let users_ref = &users;
    let user_ids: Vec<String> = ids!(users_ref);

    assert_eq!(user_ids, vec!["u1", "u2"]);
}
