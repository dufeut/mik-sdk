# mik-sql-macros

[![Crates.io](https://img.shields.io/crates/v/mik-sql-macros.svg)](https://crates.io/crates/mik-sql-macros)
[![Documentation](https://docs.rs/mik-sql-macros/badge.svg)](https://docs.rs/mik-sql-macros)

Procedural macros for [mik-sql](https://crates.io/crates/mik-sql).

## Usage

This crate is an implementation detail of `mik-sql`. You should depend on `mik-sql` directly:

```toml
[dependencies]
mik-sql = "0.1"
```

Or via `mik-sdk` (includes SQL by default):

```toml
[dependencies]
mik-sdk = "0.1"
```

## Macros Provided

| Macro         | Purpose                                        |
| ------------- | ---------------------------------------------- |
| `sql_read!`   | SELECT query with filters, sorting, pagination |
| `sql_create!` | INSERT query with returning                    |
| `sql_update!` | UPDATE query with filters                      |
| `sql_delete!` | DELETE query with filters                      |

See the [mik-sql documentation](https://docs.rs/mik-sql) for usage examples.

## License

Licensed under MIT license.
