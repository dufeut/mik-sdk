# CRUD API Example

Demonstrates REST patterns with mik-sdk:

- Method-based routing (`GET`, `POST`, `PUT`, `DELETE`)
- Path parameters (`/users/{id}`)
- Request body parsing (`req.json()`)
- HTTP status constants (`status::CREATED`, `status::NOT_FOUND`)
- RFC 7807 error responses
- Custom response headers

## Endpoints

| Method | Path         | Description    |
|--------|--------------|----------------|
| GET    | /            | API info       |
| GET    | /users       | List all users |
| POST   | /users       | Create user    |
| GET    | /users/{id}  | Get user by ID |
| PUT    | /users/{id}  | Update user    |
| DELETE | /users/{id}  | Delete user    |

## Build and Run

```bash
# Build the handler
cargo component build --release -p crud-api

# Compose with bridge
wac plug mik-bridge.wasm --plug crud_api.wasm -o service.wasm

# Run with wasmtime
wasmtime serve -S cli=y service.wasm
```

## Test

```bash
# List users
curl http://localhost:3000/users

# Get user
curl http://localhost:3000/users/1

# Create user
curl -X POST http://localhost:3000/users \
  -H "Content-Type: application/json" \
  -d '{"name": "Charlie", "email": "charlie@example.com"}'

# Update user
curl -X PUT http://localhost:3000/users/1 \
  -H "Content-Type: application/json" \
  -d '{"name": "Alice Updated"}'

# Delete user
curl -X DELETE http://localhost:3000/users/1
```
