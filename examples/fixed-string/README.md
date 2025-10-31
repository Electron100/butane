# Fixed-String Example

This example demonstrates how to use [`ArrayString`](https://docs.rs/arrayvec/latest/arrayvec/struct.ArrayString.html) from the `arrayvec` crate with Butane for memory-efficient, fixed-size string fields.

## Features Demonstrated

- **Memory Efficiency**: `ArrayString` stores string data on the stack instead of heap allocation
- **Type Safety**: Compile-time capacity checking prevents buffer overflows
- **Performance**: No heap allocations for strings within the capacity limit
- **Database Compatibility**: Works seamlessly with all Butane backends (PostgreSQL, SQLite, Turso)

## Models

### User

- `username: ArrayString<32>` - Fixed-size username (32 characters max)
- `email: ArrayString<255>` - Email address (255 characters max, RFC standard)
- `display_name: Option<ArrayString<64>>` - Optional display name (64 characters max)
- `status: ArrayString<16>` - User status (16 characters max)

### Product

- `sku: ArrayString<32>` - Product SKU as primary key (32 characters max)
- `name: ArrayString<128>` - Product name (128 characters max)
- `category: ArrayString<64>` - Product category (64 characters max)

### Order

- `order_number: ArrayString<32>` - Customer-facing order identifier
- Foreign key relationships to User and Product
- `status: ArrayString<16>` - Order status tracking

### Config

- `key: ArrayString<64>` - Configuration key as primary key
- `value: ArrayString<512>` - Configuration value
- `description: Option<ArrayString<256>>` - Optional description

## Benefits of ArrayString

1. **Stack Allocation**: No heap overhead for small to medium strings
2. **Cache Friendly**: Better memory locality compared to `String`
3. **Predictable Memory Usage**: Fixed size at compile time
4. **Zero-Copy Operations**: No allocations for operations within capacity
5. **Database Optimization**: Stored as regular TEXT columns in the database

## Usage

The example shows how to:

- Define models with `ArrayString` fields
- Handle capacity errors gracefully
- Use `ArrayString` as primary keys
- Work with optional `ArrayString` fields
- Integrate with Butane's ORM features

## Running the Example

```bash
# Generate migrations
cargo run --bin butane_cli -- migrate
# Run tests
cargo test
```

## Performance Considerations

Choose appropriate capacities for your use case:

- **Small strings** (≤64 chars): Significant performance benefits
- **Medium strings** (≤256 chars): Moderate benefits, reduced allocations
- **Large strings** (>512 chars): Consider using regular `String` for flexibility

The optimal capacity depends on your specific data patterns and performance requirements.
