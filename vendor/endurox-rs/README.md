Rust bindings for Enduro/X.

# Runtime modes

The default build has no async-runtime dependency. Use `AtmiCtx::tpcall` for
the normal blocking XATMI call path, or `AtmiCtx::tpacall` plus
`AtmiCtx::tpgetrply` when managing Enduro/X call descriptors directly.

Async support uses a runtime-neutral call state machine with an explicitly
selected reply-fd driver. Async features also enable `ctx-send`, ensuring each
adapter owns a distinct Enduro/X Object API context. Tokio-native integration
is optional:

```toml
[dependencies]
endurox-rs = { version = "0.1", features = ["tokio"] }
tokio = { version = "1", features = ["macros", "net", "rt", "time"] }
```

For an executor-independent driver backed by the `async-io` reactor instead:

```toml
endurox-rs = { version = "0.1", features = ["async-io"] }
```

The `async-io` adapter's futures can be polled by Tokio, smol, async-std, or
another standard Rust executor. Enabling both `tokio` and `async-io` is also
supported; the adapter type determines which reactor a context uses.

Async waiting requires an Enduro/X build whose `ndrx_config.h` selects
`EX_USE_EPOLL` or `EX_USE_KQUEUE`. On other queue backends adapter construction
returns `TPEINVAL`; use the blocking API from the runtime's blocking-task
facility and create the complete `AtmiCtx` inside that task.

`AtmiCtx` is deliberately `!Sync`, so its async call futures are `!Send`. Await
them directly on a current-thread runtime or a `tokio::task::LocalSet`, rather
than passing them to `tokio::spawn`:

```rust,no_run
use endurox_rs::AtmiCtx;

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let ctx = AtmiCtx::new()?;
    ctx.tpinit()?;
    let ctx = ctx.into_tokio()?;

    let request = ctx.tpalloc_carray(b"request")?;
    let mut response = ctx.tpalloc_carray(&[])?;
    ctx.tpcall("MY_SERVICE", &request, &mut response, 0).await?;

    drop(response);
    drop(request);
    ctx.tpterm()?;
    Ok(())
}
```

The async signatures match the blocking ones. There is no per-call timeout
argument, because XATMI has none: timeouts come from `NDRX_TOUT`, `tptoutset`
and `tpsblktime`, and the async path reads the effective value with
`tpgblktime` before each `tpacall`. For a one-off override, use the same idiom
you would use before a blocking call:

```rust,no_run
# use endurox_rs::{AtmiCtx, TPBLK_NEXT};
# async fn f(ctx: &endurox_rs::TokioAtmiCtx, req: &endurox_rs::TypedBuffer<'_>,
#            rsp: &mut endurox_rs::TypedBuffer<'_>) -> Result<(), endurox_rs::AtmiError> {
ctx.tpsblktime(3, TPBLK_NEXT)?;
ctx.tpcall("MY_SERVICE", req, rsp, 0).await?;
# Ok(())
# }
```

Concurrent async calls on one context share a single reactor registration.
Replies are demultiplexed by call descriptor using `tpgetrply(TPGETANY)`, so a
reply wakes exactly the future waiting for it, and no reply is ever diverted
into Enduro/X's in-memory queue where the reply fd could not signal it again.

Dropping `AsyncAtmiCtx::tpcall` cancels its Enduro/X call descriptor; dropping
`AsyncAtmiCtx::tpgetrply` leaves its caller-owned descriptor pending so it can
be awaited again or cancelled explicitly. Multiple adapters may share one
executor thread; each owns a separate Enduro/X context and reply queue.

The generic form is `AsyncAtmiCtx<D>`, where `D: AsyncReplyDriver`. The provided
drivers are `TokioReplyDriver` and `AsyncIoReplyDriver`. Because the adapter
owns `AtmiCtx`, a context cannot accidentally be registered with two reactors.

# Server dispatch threads

`AtmiCtx::tp_run` enables Enduro/X's multithread-capable integration mode. When
`maxdispatchthreads` is greater than one, `mindispatchthreads` worker threads
are created and service callbacks may execute concurrently on any of them.
Each callback receives a worker-local `AtmiCtx`; with `ctx-send` it temporarily
uses that worker's OAPI context and restores the worker TLS before returning.
With `maxdispatchthreads=1`, callbacks stay on the `tp_run` main thread and use
the owning server context; worker init/done hooks are intentionally not called.
With `mindispatchthreads=1` and `maxdispatchthreads>1`, one worker is created and
its thread init/done hooks are called normally.

Rust integration also uses `ATMI_SRVLIB_NOLONGJUMP`, so `tpreturn` and
`tpforward` return through Rust normally instead of performing a C `longjmp`
across Rust stack frames. Service handlers must protect any shared application
state because the same handler function can run concurrently.

# Testing

Sync mode tests:

```
$ cargo test
```

Context-migration tests:

```
$ cargo test --features ctx-send
```

Tokio API tests:

```
$ cargo test --features tokio
```

Executor-independent async API tests:

```
$ cargo test --features async-io
```
