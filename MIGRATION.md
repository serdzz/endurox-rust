# Migration: endurox-sys → official endurox-rs

This branch (`migrate-to-endurox-rs`) ports every example server and client in
this repository from the in-tree `endurox-sys` bindings to the official
[endurox-rs](https://github.com/endurox-dev/endurox-rs) bindings maintained by
the Enduro/X authors.

Service names, UBF fields (`ubftab/test.fd`), XA semantics and observable
behavior are unchanged. This is a bindings swap, not a redesign.

## What was migrated

| Crate | Services | Commit |
| --- | --- | --- |
| `samplesvr_rust` | ECHO, HELLO, STATUS, DATAPROC, TRANSACTION | `6abb13b` |
| `ubfsvr_rust` | UBFECHO, UBFTEST, UBFADD, UBFGET | `6abb13b` |
| `ubf_test_client` | client for the UBF services | `6abb13b` |
| `rest_gateway` | Actix REST façade over all of the above | `6abb13b` |
| `oracle_txn_server` | CREATE_TXN, GET_TXN, LIST_TXN (XA) | `a9e1344` |

The official bindings are vendored under `vendor/endurox-rs`
(commit `3b16530`) — see “Why vendored” below.

## API mapping

The old `endurox-sys` surface and its endurox-rs replacement:

| endurox-sys | endurox-rs |
| --- | --- |
| raw `extern "C"` service handlers, `static mut` state | `AtmiCtx::tp_run` + `ServerHooks`, state in `OnceLock` |
| hand-rolled `tpreturn` FFI | safe `TpSvcInfo` callbacks, `tpreturn_ubf` |
| manual `Bchg`/`Bget` FFI on raw buffers | `TypedUbf` (`bchg`, `bget_*`, `bprint`) |
| `#[derive(UbfStruct)]` (endurox-derive) | `UbfSerialize` / `UbfDeserialize` derives + `ubf_serde` |
| raw `tpbegin`/`tpcommit`/`tpabort` FFI + `tperrno` | `ctx.tpbegin` / `ctx.tpcommit` / `ctx.tpabort` / `ctx.tpgetlev` |
| generated UBF constants via endurox-sys build script | per-crate `build.rs` generating constants from `ubftab/*.fd.h` |

### rest_gateway threading model

`AtmiCtx` is deliberately `!Send`/`!Sync` in endurox-rs, so its futures cannot
cross threads. The gateway therefore keeps a **thread-local `AtmiCtx` per Actix
worker** and issues blocking `tpcall`s from the worker thread — the approach
endurox-rs documents for non-async integrations. Migrating to the
`tokio`/`async-io` reply-driver features is possible later (the installed
Enduro/X deb is the `EX_USE_EPOLL` build they require); see Future work.

### oracle_txn_server: the `oracle` feature

`diesel-oci` needs the Oracle client libraries at **build time**. They are not
present in a stock CI/dev environment, so the Oracle backend moved behind an
opt-in cargo feature:

```toml
# default: PostgreSQL only
cargo build --release -p oracle_txn_server
# with Oracle client libs installed:
cargo build --release -p oracle_txn_server --features oracle
```

Without the feature, an `oracle://` `DATABASE_URL` produces a clear runtime
error instead of a link failure. `diesel` now lists its `chrono` feature
explicitly (it was previously pulled in transitively via `diesel-oci`).

## Why vendored (and the three compat patches)

Upstream endurox-rs targets Enduro/X **master** headers; this repo runs against
released **Enduro/X 8.0.10**. The bindings are vendored at
`vendor/endurox-rs` with a `[patch]` redirect in the workspace `Cargo.toml`:

```toml
[patch.'https://github.com/endurox-dev/endurox-rs']
endurox-rs = { path = "vendor/endurox-rs" }
```

Local patches (all candidates for upstreaming):

1. **`TPQKEEPORIG` fallback define** — the constant is absent from 8.0.10
   headers.
2. **Two const-pointer casts** — `tpadvertise_full` / `tplogprintubf`
   signatures differ between 8.0.10 and master.
3. **`mkfldhdr` Rust mode shim** — endurox-rs's build script invokes
   `mkfldhdr -m4` (Rust output), which 8.0.10's `mkfldhdr` does not have.
   `tools/mkfldhdr-rust` is a drop-in shim implementing exactly that mode,
   wired via `.cargo/config.toml`:

   ```toml
   [env]
   ENDUROX_MKFLDHDR = { value = "tools/mkfldhdr-rust", relative = true }
   ```

Once these land upstream (or Enduro/X ships `mkfldhdr -m4`), the vendor copy,
the `[patch]` redirect and the shim can all be deleted in favor of a plain git
or crates.io dependency.

## Linker workaround

Enduro/X 8.0.10 shared libraries do not declare all inter-library `DT_NEEDED`
entries (e.g. `libubf` uses `libnstd` symbols without linking it). With the
default `--as-needed`, the linker prunes `libnstd` and binaries die at startup
with:

```
undefined symbol: G_ndrx_debug
```

Fixed globally in `.cargo/config.toml`:

```toml
[target.'cfg(target_os = "linux")']
rustflags = [
    "-C", "link-arg=-Wl,--no-as-needed",
    "-C", "link-arg=-lnstd",
    "-C", "link-arg=-lubf",
    "-C", "link-arg=-latmi",
]
```

## Build prerequisites

- Enduro/X 8.0.10 installed system-wide (`/usr/include/atmi.h`,
  `/usr/lib/libatmi.so`) — the `gnu_epoll` deb in this repo works
- `pkg-config` and `libclang-dev` (bindgen)
- `libpq-dev` (PostgreSQL backend of oracle_txn_server)
- Rust stable or nightly

```sh
cargo build --release \
  -p samplesvr_rust -p ubfsvr_rust -p ubf_test_client \
  -p rest_gateway -p oracle_txn_server
```

All five crates build cleanly from scratch (verified after `cargo clean`).

## Verification performed

Everything below was executed against a **live Enduro/X 8.0.10 domain**
(`ndrxd` started with this repo's `conf/`), not just compiled:

- `xadmin psc` — all services AVAIL, including CREATE_TXN / GET_TXN / LIST_TXN
- `ubf_test_client` — all four UBF tests pass against the ported
  `ubfsvr_rust` (UBFADD, UBFTEST, UBFECHO, UBFGET)
- `rest_gateway` — `/api/status`, `/api/hello`, `/api/transaction` return
  correct responses backed by `samplesvr_rust`
- **End-to-end XA path with PostgreSQL**:
  `curl POST /api/oracle/create` → `tpcall("CREATE_TXN")` →
  `oracle_txn_server` XA transaction → row verified in the `transactions`
  table (`sale, ACC-001, 15050 EUR, SUCCESS`); input validation confirmed
  (non-`sale` type returns `INVALID_TYPE`)

## Future work

- A second REST gateway now exists: `rest_axum_gateway` (axum + tokio,
  port 8081 by default). It serves the same endpoints and JSON shapes as the
  Actix `rest_gateway`, calling Enduro/X through a bounded `spawn_blocking`
  pool of thread-local `AtmiCtx`s (`REST_WORKERS` controls the pool size).
- Upstream the three compat patches to `endurox-dev/endurox-rs`, then drop
  `vendor/endurox-rs`, the `[patch]` redirect and `tools/mkfldhdr-rust`.
- `endurox-sys` and `endurox-derive` are no longer used by any crate and can
  be removed from the workspace.
- Validate the `--features oracle` build on a machine with Oracle client
  libraries (the diesel-oci path is cfg-gated, not compiled here).
- Optionally move `rest_gateway` to the endurox-rs `tokio`/`async-io`
  reply-driver (requires the `EX_USE_EPOLL` Enduro/X build, which the
  bundled deb already is).
