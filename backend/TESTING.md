# Testing Guide

This guide covers how to run and write tests for the Ticket Arcade backend.

---

## Prerequisites

- Rust 2024 edition (1.75+)
- Docker Desktop (for PostgreSQL)
- sqlx-cli installed

```bash
cargo install sqlx-cli --no-default-features --features postgres
```

---

## Running Tests

### Unit Tests

```bash
cd backend
cargo test
```

### With Logging

```bash
RUST_LOG=debug cargo test -- --nocapture
```

### Specific Test

```bash
cargo test test_name_here
```

---

## Test Categories

### 1. Unit Tests

Located in `src/*.rs` as `#[cfg(test)]` modules. Test individual functions in isolation.

**Example areas:**
- Query parameter validation
- Ethereum address parsing
- Status enum conversion

### 2. Integration Tests

Located in `tests/` directory (if present). Test API endpoints with a real database.

**Setup:**
```bash
# Start test database
docker compose up -d

# Run migrations
sqlx migrate run --source migrations

# Run integration tests
cargo test --test '*'
```

---

## Database Testing

### Reset Test Data

```bash
# Connect to Postgres
docker exec -it project-postgres-1 psql -U postgres -d raffle_backend

# Clear all data
TRUNCATE raffles, purchases, randomness_requests, randomness_fulfillments, indexer_state CASCADE;
```

### Reset Indexer State

```sql
UPDATE indexer_state SET last_processed_block = 0;
```

---

## Manual API Testing

### Using curl

```bash
# Health check
curl http://localhost:8080/health

# List raffles
curl "http://localhost:8080/v1/raffles?limit=10"

# Get specific raffle
curl http://localhost:8080/v1/raffles/1

# Get purchases
curl "http://localhost:8080/v1/raffles/1/purchases?limit=100"

# Get proof
curl http://localhost:8080/v1/raffles/1/proof

# List randomness requests
curl "http://localhost:8080/v1/randomness/requests?limit=10"

# Get specific randomness request
curl http://localhost:8080/v1/randomness/requests/1
```

### Using httpie

```bash
# Health check
http :8080/health

# List raffles
http :8080/v1/raffles limit==10 status==ACTIVE
```

---

## Code Quality

### Linting

```bash
cargo clippy -- -D warnings
```

### Formatting

```bash
cargo fmt --check  # Check only
cargo fmt          # Auto-fix
```

### Build Check

```bash
cargo build --release
```

---

## Debugging

### Enable Debug Logs

```bash
RUST_LOG=debug cargo run
```

### Log Levels

| Level | Use Case |
|-------|----------|
| `error` | Critical failures |
| `warn` | Recoverable issues |
| `info` | Normal operation (default) |
| `debug` | Detailed flow |
| `trace` | RPC calls, SQL queries |

### Component-Specific Logs

```bash
# Indexer only
RUST_LOG=backend::indexer=debug cargo run

# API only
RUST_LOG=backend::api=debug cargo run

# Both
RUST_LOG=backend::indexer=debug,backend::api=debug cargo run
```

---

## Common Issues

| Problem | Solution |
|---------|----------|
| "Connection refused" | Run `docker compose up -d` |
| "Relation does not exist" | Run `sqlx migrate run --source migrations` |
| Tests timeout | Check database is healthy with `docker compose logs postgres` |
| Stale data | Reset indexer state (see above) |
| RPC errors | Verify `RPC_URL` in `.env` |

---

## CI/CD Considerations

For automated testing:

```bash
# Full check suite
cargo fmt --check && cargo clippy -- -D warnings && cargo test
```

Ensure `DATABASE_URL` points to a test database, not production.

---

## Writing New Tests

### Unit Test Template

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_example() {
        let result = some_function();
        assert_eq!(result, expected_value);
    }

    #[tokio::test]
    async fn test_async_example() {
        let result = async_function().await;
        assert!(result.is_ok());
    }
}
```

### Test Best Practices

1. **Isolate tests** - Each test should set up its own data
2. **Use descriptive names** - `test_buy_tickets_exceeds_max_returns_error`
3. **Test edge cases** - Empty results, max limits, invalid input
4. **Clean up** - Reset state between tests when using shared resources
