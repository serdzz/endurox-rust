use crate::{raw, AtmiCtx, AtmiError, AtmiResult, TpSvcInfo, TypedBuffer, TypedUbf};
use core::ffi::{c_char, c_int, c_long};
use std::ffi::{CStr, CString};
use std::{
    collections::HashMap,
    panic::{catch_unwind, AssertUnwindSafe},
    sync::{Mutex, OnceLock},
};

/// Low-level C-compatible service callback used by Enduro/X registration APIs.
type ServiceCallback = unsafe extern "C" fn(*mut raw::TPSVCINFO);
type PollerCallback = unsafe extern "C" fn(c_int, u32, *mut ::std::os::raw::c_void) -> c_int;
type PeriodCallback = unsafe extern "C" fn() -> c_int;
type BeforePollCallback = unsafe extern "C" fn() -> c_int;
type ServerInitHook = unsafe extern "C" fn(c_int, *mut *mut c_char) -> c_int;
type ServerDoneHook = unsafe extern "C" fn();

/// High-level server init callback.
///
/// Receives the server's command-line arguments as Enduro/X delivers them
/// to the C-level `tpsvrinit(int argc, char **argv)` — i.e. only the user
/// arguments after the framework's own option processing (typically what
/// follows the `--` separator in `CLOPT`). The first element is the program
/// name when present.
///
/// Returning `Err(...)` aborts server startup.
pub type RustServerInitHook = fn(&AtmiCtx, &[String]) -> AtmiResult<()>;

/// High-level server shutdown callback.
pub type RustServerDoneHook = fn(&AtmiCtx);

/// Per-worker-thread init callback, mirroring C `tpsvrthrinit`.
///
/// Runs once on each libatmisrv dispatch thread, after Enduro/X has opened that
/// worker's ATMI session and before it takes any request, so the supplied
/// context is that worker's own. Use it for per-thread resources -- a database
/// handle, a thread-local cache -- which have nowhere else to live: the context
/// a service handler receives is built per dispatch, not per thread.
///
/// Only called when dispatch threading is active (`maxdispatchthreads > 1`).
/// Returning `Err(...)` aborts that worker's startup.
pub type RustServerThreadInitHook = fn(&AtmiCtx, &[String]) -> AtmiResult<()>;

/// Per-worker-thread shutdown callback, mirroring C `tpsvrthrdone`.
///
/// Runs on the worker thread before Enduro/X terminates its ATMI session, so
/// the context is still usable.
pub type RustServerThreadDoneHook = fn(&AtmiCtx);

/// The four C server lifecycle hooks, as one value.
///
/// Only `tpsvrinit` is mandatory; the rest default to absent. Enduro/X's own
/// `tpsvrthrinit`/`tpsvrthrdone` defaults still run either way -- they perform
/// the worker's `tx_open()`/`tx_close()` -- and a Rust thread hook runs in
/// addition to them, not instead.
#[derive(Debug, Clone, Copy)]
pub struct ServerHooks {
    init: RustServerInitHook,
    done: Option<RustServerDoneHook>,
    thread_init: Option<RustServerThreadInitHook>,
    thread_done: Option<RustServerThreadDoneHook>,
}

impl ServerHooks {
    /// Start from the mandatory `tpsvrinit` hook.
    pub fn new(init: RustServerInitHook) -> Self {
        Self {
            init,
            done: None,
            thread_init: None,
            thread_done: None,
        }
    }

    /// Set the `tpsvrdone` hook.
    pub fn done(mut self, done: RustServerDoneHook) -> Self {
        self.done = Some(done);
        self
    }

    /// Set the per-worker-thread `tpsvrthrinit` hook.
    pub fn thread_init(mut self, thread_init: RustServerThreadInitHook) -> Self {
        self.thread_init = Some(thread_init);
        self
    }

    /// Set the per-worker-thread `tpsvrthrdone` hook.
    pub fn thread_done(mut self, thread_done: RustServerThreadDoneHook) -> Self {
        self.thread_done = Some(thread_done);
        self
    }
}

/// High-level service callback used by [`AtmiCtx::tpadvertise`].
///
/// When Enduro/X dispatch threading is configured, the same function may be
/// invoked concurrently on several libatmisrv worker threads. Any application
/// state accessed by the handler must therefore be thread-safe.
pub type RustServiceCallback = for<'ctx> fn(&'ctx AtmiCtx, &mut TpSvcInfo<'ctx>);

/// Event delivered to a Rust poller callback registered with
/// [`AtmiCtx::tpext_addpollerfd`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PollerEvent {
    pub fd: i32,
    pub events: u32,
    pub user_data: usize,
}

/// High-level poller callback used by [`AtmiCtx::tpext_addpollerfd`].
/// The context is the owning main server context because extension events are
/// always dispatched by the main poll loop, never by service workers.
pub type RustPollerCallback = for<'ctx> fn(&'ctx AtmiCtx, PollerEvent) -> i32;

/// High-level periodic callback used by [`AtmiCtx::tpext_addperiodcb`].
/// The callback receives the owning main server context.
pub type RustPeriodCallback = for<'ctx> fn(&'ctx AtmiCtx) -> i32;

/// High-level before-poll callback used by [`AtmiCtx::tpext_addb4pollcb`].
/// The callback receives the owning main server context.
pub type RustBeforePollCallback = for<'ctx> fn(&'ctx AtmiCtx) -> i32;

/// Service return status for [`AtmiCtx::tpreturn`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TpReturnStatus {
    Success,
    Fail,
}

impl TpReturnStatus {
    #[inline]
    fn to_raw(self) -> c_int {
        match self {
            TpReturnStatus::Success => raw::TPSUCCESS as c_int,
            TpReturnStatus::Fail => raw::TPFAIL as c_int,
        }
    }
}

#[derive(Default)]
struct ServerRuntime {
    ctx_addr: usize,
    main_thread_id: Option<std::thread::ThreadId>,
    init_hook: Option<RustServerInitHook>,
    done_hook: Option<RustServerDoneHook>,
    thread_init_hook: Option<RustServerThreadInitHook>,
    thread_done_hook: Option<RustServerThreadDoneHook>,
    init_error: Option<AtmiError>,
    /// First worker-thread init failure, kept so a startup abort is
    /// diagnosable rather than surfacing as a bare EXFAIL.
    thread_init_error: Option<AtmiError>,
    services: HashMap<String, RustServiceCallback>,
}

impl ServerRuntime {
    fn reset(&mut self) {
        self.ctx_addr = 0;
        self.main_thread_id = None;
        self.init_hook = None;
        self.done_hook = None;
        self.thread_init_hook = None;
        self.thread_done_hook = None;
        self.init_error = None;
        self.thread_init_error = None;
        self.services.clear();
    }
}

static SERVER_RUNTIME: OnceLock<Mutex<ServerRuntime>> = OnceLock::new();

#[derive(Default)]
struct ExtensionRuntime {
    pollers: HashMap<i32, (RustPollerCallback, usize)>,
    period_cb: Option<RustPeriodCallback>,
    before_poll_cb: Option<RustBeforePollCallback>,
}

static EXTENSION_RUNTIME: OnceLock<Mutex<ExtensionRuntime>> = OnceLock::new();

#[inline]
fn server_runtime() -> &'static Mutex<ServerRuntime> {
    SERVER_RUNTIME.get_or_init(|| Mutex::new(ServerRuntime::default()))
}

#[inline]
fn extension_runtime() -> &'static Mutex<ExtensionRuntime> {
    EXTENSION_RUNTIME.get_or_init(|| Mutex::new(ExtensionRuntime::default()))
}

#[inline]
fn runtime_lock_err() -> AtmiError {
    AtmiError::new(raw::TPESYSTEM, "server runtime state is poisoned")
}

struct ServerRuntimeGuard;

impl Drop for ServerRuntimeGuard {
    fn drop(&mut self) {
        if let Ok(mut rt) = server_runtime().lock() {
            rt.reset();
        }
    }
}

struct ServerThreadModeGuard {
    previous: c_int,
    previous_thread_init: Option<ServerInitHook>,
    previous_thread_done: Option<ServerDoneHook>,
}

impl ServerThreadModeGuard {
    unsafe fn enable() -> Self {
        let previous = raw::_tmbuilt_with_thread_option;
        let previous_thread_init = raw::ndrx_G_tpsvrthrinit;
        let previous_thread_done = raw::ndrx_G_tpsvrthrdone;
        raw::_tmbuilt_with_thread_option = 1;
        // Always our own trampolines: they chain to Enduro/X's defaults for
        // tx_open()/tx_close() and additionally dispatch any Rust thread hook.
        raw::ndrx_G_tpsvrthrinit = Some(rust_thread_init);
        raw::ndrx_G_tpsvrthrdone = Some(rust_thread_done);
        Self {
            previous,
            previous_thread_init,
            previous_thread_done,
        }
    }
}

impl Drop for ServerThreadModeGuard {
    fn drop(&mut self) {
        unsafe {
            raw::_tmbuilt_with_thread_option = self.previous;
            raw::ndrx_G_tpsvrthrinit = self.previous_thread_init;
            raw::ndrx_G_tpsvrthrdone = self.previous_thread_done;
        }
    }
}

unsafe fn fail_current_service(svc_ptr: *mut raw::TPSVCINFO) {
    let data = (*svc_ptr).data;
    let len = (*svc_ptr).len.max(0) as c_long;
    raw::tpreturn(TpReturnStatus::Fail.to_raw(), 0, data, len, 0);
}

unsafe extern "C" fn rust_service_dispatch(svc_ptr: *mut raw::TPSVCINFO) {
    if svc_ptr.is_null() {
        return;
    }

    let current_thread = std::thread::current().id();
    let (ctx_addr, is_main_thread) = match server_runtime().lock() {
        Ok(rt) => (
            rt.ctx_addr,
            rt.main_thread_id
                .as_ref()
                .is_some_and(|thread_id| *thread_id == current_thread),
        ),
        Err(_) => {
            fail_current_service(svc_ptr);
            return;
        }
    };
    if ctx_addr == 0 {
        fail_current_service(svc_ptr);
        return;
    }

    // Single-threaded servers dispatch on the tp_run thread, where the owning
    // context is already correct. Threaded servers dispatch on initialized
    // libatmisrv workers and need a callback-scoped worker context instead.
    let worker_ctx = if is_main_thread {
        None
    } else {
        match AtmiCtx::borrow_current_worker() {
            Ok(ctx) => Some(ctx),
            Err(_) => {
                fail_current_service(svc_ptr);
                return;
            }
        }
    };
    let ctx = worker_ctx
        .as_ref()
        .unwrap_or_else(|| &*(ctx_addr as *const AtmiCtx));
    let mut svc = TpSvcInfo::from_raw(ctx, svc_ptr);

    let key = if svc.fname().is_empty() {
        svc.name().to_owned()
    } else {
        svc.fname().to_owned()
    };

    let cb = match server_runtime().lock() {
        Ok(rt) => rt
            .services
            .get(&key)
            .copied()
            .or_else(|| rt.services.get(svc.name()).copied()),
        Err(_) => None,
    };

    match cb {
        Some(handler) => {
            if catch_unwind(AssertUnwindSafe(|| handler(ctx, &mut svc))).is_err() {
                // Handler panicked. If it hadn't consumed the data buffer yet,
                // use it for the error response; otherwise allocate a fresh one.
                let err_ptr = match svc.take_data() {
                    Some(buf) => buf.into_raw(),
                    None => ctx
                        .tpalloc_ubf(256)
                        .map(|u| u.into_inner().into_raw())
                        .unwrap_or(std::ptr::null_mut()),
                };
                ctx.tpreturn_raw(TpReturnStatus::Fail.to_raw(), 0, err_ptr, 0, 0);
            }
        }
        None => {
            let err_ptr = svc
                .take_data()
                .map(|b| b.into_raw())
                .unwrap_or(std::ptr::null_mut());
            ctx.tpreturn_raw(TpReturnStatus::Fail.to_raw(), 0, err_ptr, 0, 0);
        }
    }
}

unsafe extern "C" fn rust_poller_dispatch(
    fd: c_int,
    events: u32,
    _ptr1: *mut ::std::os::raw::c_void,
) -> c_int {
    let ctx = match main_extension_context() {
        Some(ctx) => ctx,
        None => return -1,
    };
    let (cb, user_data) = match extension_runtime().lock() {
        Ok(rt) => match rt.pollers.get(&(fd as i32)).copied() {
            Some(v) => v,
            None => return 0,
        },
        Err(_) => return -1,
    };

    catch_unwind(AssertUnwindSafe(|| {
        cb(
            ctx,
            PollerEvent {
                fd: fd as i32,
                events,
                user_data,
            },
        )
    }))
    .unwrap_or(-1)
}

unsafe extern "C" fn rust_period_dispatch() -> c_int {
    let ctx = match main_extension_context() {
        Some(ctx) => ctx,
        None => return -1,
    };
    let cb = match extension_runtime().lock() {
        Ok(rt) => rt.period_cb,
        Err(_) => return -1,
    };

    match cb {
        Some(cb) => catch_unwind(AssertUnwindSafe(|| cb(ctx))).unwrap_or(-1),
        None => 0,
    }
}

unsafe extern "C" fn rust_before_poll_dispatch() -> c_int {
    let ctx = match main_extension_context() {
        Some(ctx) => ctx,
        None => return -1,
    };
    let cb = match extension_runtime().lock() {
        Ok(rt) => rt.before_poll_cb,
        Err(_) => return -1,
    };

    match cb {
        Some(cb) => catch_unwind(AssertUnwindSafe(|| cb(ctx))).unwrap_or(-1),
        None => 0,
    }
}

/// Resolve the `tp_run` context for an extension callback, or `None` when it is
/// unavailable or the caller is not the main poll-loop thread.
///
/// # Safety
///
/// The returned `'static` lifetime is wider than the context actually lives:
/// `ctx_addr` points at a stack local owned by [`AtmiCtx::tp_run`]. Two
/// invariants keep it from escaping. `ServerRuntimeGuard` clears `ctx_addr`
/// before `tp_run` returns, so a stale address is never resolved, and the
/// extension callback types are higher-ranked (`for<'ctx> fn(&'ctx AtmiCtx,
/// ..)`), so a callback cannot store the reference it receives. Callers must
/// therefore only narrow this lifetime into a callback invocation and never
/// retain the reference past that call.
/// Reject a poll-extension mutation issued from anywhere but the main thread.
///
/// Enduro/X keeps these in one unsynchronised global list that the main
/// dispatch thread walks on every poll iteration, so mutating it from a worker
/// corrupts the poll set and wedges the server. That is a documented
/// constraint, but documentation cannot stop a safe method being called from a
/// service handler under `maxdispatchthreads > 1` -- so it is checked.
///
/// Only enforced while a server is actually running: outside `tp_run` there is
/// no main thread to compare against and no poll loop to race.
fn require_main_thread(what: &str) -> AtmiResult<()> {
    let current = std::thread::current().id();
    let main = match server_runtime().lock() {
        Ok(rt) => rt.main_thread_id,
        Err(_) => return Err(runtime_lock_err()),
    };
    match main {
        Some(main) if main != current => Err(AtmiError::new(
            raw::TPEPROTO,
            format!(
                "{what} must be called from the main server thread; calling it                  from a dispatch worker races Enduro/X's global poll-extension                  list and can wedge the server"
            ),
        )),
        _ => Ok(()),
    }
}

unsafe fn main_extension_context() -> Option<&'static AtmiCtx> {
    let current_thread = std::thread::current().id();
    let ctx_addr = match server_runtime().lock() {
        Ok(rt)
            if rt.ctx_addr != 0
                && rt
                    .main_thread_id
                    .as_ref()
                    .is_some_and(|thread_id| *thread_id == current_thread) =>
        {
            rt.ctx_addr
        }
        _ => return None,
    };
    Some(&*(ctx_addr as *const AtmiCtx))
}

/// Collect `argc`/`argv` as Enduro/X hands them to a C hook.
unsafe fn hook_args(argc: c_int, argv: *mut *mut c_char) -> Vec<String> {
    if argv.is_null() || argc <= 0 {
        return Vec::new();
    }
    (0..argc as usize)
        .filter_map(|i| {
            let p = *argv.add(i);
            if p.is_null() {
                None
            } else {
                Some(CStr::from_ptr(p).to_string_lossy().into_owned())
            }
        })
        .collect()
}

/// `tpsvrthrinit` trampoline, run on each dispatch thread.
///
/// Enduro/X calls this from `ndrx_call_tpsvrthrinit`, which has already done
/// `tpinit(NULL)` for the worker, so the thread has a usable ATMI context. The
/// library default runs first for its `tx_open()`; a Rust hook runs after it and
/// on top of it, never instead.
unsafe extern "C" fn rust_thread_init(argc: c_int, argv: *mut *mut c_char) -> c_int {
    /// Keep the first failure only: later workers may fail for knock-on
    /// reasons, and the first one is the useful diagnosis.
    fn record(err: AtmiError) -> c_int {
        if let Ok(mut rt) = server_runtime().lock() {
            if rt.thread_init_error.is_none() {
                rt.thread_init_error = Some(err);
            }
        }
        raw::EXFAIL as c_int
    }

    if raw::tpsvrthrinit(argc, argv) < 0 {
        return record(AtmiError::new(
            raw::TPESYSTEM,
            "Enduro/X default tpsvrthrinit() failed for a dispatch thread",
        ));
    }

    let hook = match server_runtime().lock() {
        Ok(rt) => rt.thread_init_hook,
        Err(_) => return record(runtime_lock_err()),
    };
    let Some(hook) = hook else {
        return raw::EXSUCCEED as c_int;
    };

    let ctx = match AtmiCtx::borrow_current_worker() {
        Ok(ctx) => ctx,
        Err(err) => return record(err),
    };
    let args = hook_args(argc, argv);

    match catch_unwind(AssertUnwindSafe(|| hook(&ctx, &args))) {
        Ok(Ok(())) => raw::EXSUCCEED as c_int,
        Ok(Err(err)) => record(err),
        Err(_) => record(AtmiError::new(
            raw::TPESYSTEM,
            "server thread init hook panicked",
        )),
    }
}

/// `tpsvrthrdone` trampoline, run on each dispatch thread.
///
/// `ndrx_call_tpsvrthrdone` invokes this *before* `tpterm()`, so the worker
/// context is still live. The Rust hook runs first, then the library default
/// for its `tx_close()`.
unsafe extern "C" fn rust_thread_done() {
    let hook = match server_runtime().lock() {
        Ok(rt) => rt.thread_done_hook,
        Err(_) => None,
    };

    if let Some(hook) = hook {
        if let Ok(ctx) = AtmiCtx::borrow_current_worker() {
            let _ = catch_unwind(AssertUnwindSafe(|| hook(&ctx)));
        }
    }

    raw::tpsvrthrdone();
}

unsafe extern "C" fn rust_server_init(argc: c_int, argv: *mut *mut c_char) -> c_int {
    let (ctx_addr, init_hook) = match server_runtime().lock() {
        Ok(rt) => (rt.ctx_addr, rt.init_hook),
        Err(_) => return raw::EXFAIL as c_int,
    };

    if ctx_addr == 0 {
        return raw::EXFAIL as c_int;
    }

    let Some(init_cb) = init_hook else {
        return raw::EXFAIL as c_int;
    };

    let args: Vec<String> = if argv.is_null() || argc <= 0 {
        Vec::new()
    } else {
        (0..argc as usize)
            .filter_map(|i| {
                let p = *argv.add(i);
                if p.is_null() {
                    None
                } else {
                    Some(CStr::from_ptr(p).to_string_lossy().into_owned())
                }
            })
            .collect()
    };

    let ctx = &*(ctx_addr as *const AtmiCtx);
    match catch_unwind(AssertUnwindSafe(|| init_cb(ctx, &args))) {
        Ok(Ok(())) => raw::EXSUCCEED as c_int,
        Ok(Err(err)) => {
            if let Ok(mut rt) = server_runtime().lock() {
                rt.init_error = Some(err);
            }
            raw::EXFAIL as c_int
        }
        Err(_) => {
            if let Ok(mut rt) = server_runtime().lock() {
                rt.init_error = Some(AtmiError::new(
                    raw::TPESYSTEM,
                    "panic in server init callback",
                ));
            }
            raw::EXFAIL as c_int
        }
    }
}

unsafe extern "C" fn rust_server_done() {
    let (ctx_addr, done_hook) = match server_runtime().lock() {
        Ok(rt) => (rt.ctx_addr, rt.done_hook),
        Err(_) => return,
    };

    if ctx_addr == 0 {
        return;
    }

    let Some(done_cb) = done_hook else {
        return;
    };

    let ctx = &*(ctx_addr as *const AtmiCtx);
    let _ = catch_unwind(AssertUnwindSafe(|| done_cb(ctx)));
}

impl AtmiCtx {
    #[inline]
    pub(crate) fn rc_to_result(&self, rc: c_int) -> AtmiResult<()> {
        if rc == raw::EXSUCCEED as c_int {
            Ok(())
        } else {
            Err(self.atmi_last_error())
        }
    }

    pub fn tpadvertise_full(
        &self,
        svc_nm: &str,
        p_func: Option<ServiceCallback>,
        fn_nm: &str,
    ) -> AtmiResult<()> {
        let c_svc = CString::new(svc_nm).map_err(|_| self.atmi_last_error())?;
        let c_fn = CString::new(fn_nm).map_err(|_| self.atmi_last_error())?;

        #[cfg(not(feature = "ctx-send"))]
        let rc = unsafe { raw::tpadvertise_full(c_svc.as_ptr() as _, p_func, c_fn.as_ptr() as _) };

        #[cfg(feature = "ctx-send")]
        let rc = unsafe {
            raw::Otpadvertise_full(
                self.c_ctx_ptr(),
                c_svc.as_ptr() as *mut c_char,
                p_func,
                c_fn.as_ptr() as *mut c_char,
            )
        };

        self.rc_to_result(rc)
    }

    /// High-level Rust service advertisement.
    ///
    /// This avoids `extern "C"` callbacks in user code. Register these from
    /// the `tp_run(...)` init hook.
    pub fn tpadvertise(&self, svc_nm: &str, handler: RustServiceCallback) -> AtmiResult<()> {
        let self_addr = self as *const AtmiCtx as usize;
        {
            let mut rt = server_runtime().lock().map_err(|_| runtime_lock_err())?;
            if rt.ctx_addr != self_addr {
                return Err(AtmiError::new(
                    raw::TPEPROTO,
                    "tpadvertise() must be called from the active tp_run() context",
                ));
            }
            rt.services.insert(svc_nm.to_owned(), handler);
        }

        if let Err(err) = self.tpadvertise_full(svc_nm, Some(rust_service_dispatch), svc_nm) {
            if let Ok(mut rt) = server_runtime().lock() {
                rt.services.remove(svc_nm);
            }
            return Err(err);
        }
        Ok(())
    }

    pub fn tpunadvertise(&self, svc_nm: &str) -> AtmiResult<()> {
        let c_svc = CString::new(svc_nm).map_err(|_| self.atmi_last_error())?;
        let self_addr = self as *const AtmiCtx as usize;

        #[cfg(not(feature = "ctx-send"))]
        let rc = unsafe { raw::tpunadvertise(c_svc.as_ptr() as *mut c_char) };

        #[cfg(feature = "ctx-send")]
        let rc = unsafe { raw::Otpunadvertise(self.c_ctx_ptr(), c_svc.as_ptr() as *mut c_char) };

        self.rc_to_result(rc)?;
        if let Ok(mut rt) = server_runtime().lock() {
            if rt.ctx_addr == self_addr {
                rt.services.remove(svc_nm);
            }
        }
        Ok(())
    }

    unsafe fn tpreturn_raw(
        &self,
        rval: i32,
        rcode: i64,
        data: *mut c_char,
        len: usize,
        flags: i64,
    ) {
        #[cfg(not(feature = "ctx-send"))]
        raw::tpreturn(
            rval as c_int,
            rcode as c_long,
            data,
            len as c_long,
            flags as c_long,
        );

        #[cfg(feature = "ctx-send")]
        raw::Otpreturn(
            self.c_ctx_ptr(),
            rval as c_int,
            rcode as c_long,
            data,
            len as c_long,
            flags as c_long,
        );
    }

    pub(crate) unsafe fn tpforward(&self, svc: &str, data: *mut c_char, len: usize, flags: i64) {
        let c_svc = match CString::new(svc) {
            Ok(s) => s,
            Err(_) => return,
        };

        #[cfg(not(feature = "ctx-send"))]
        raw::tpforward(
            c_svc.as_ptr() as *mut c_char,
            data,
            len as c_long,
            flags as c_long,
        );

        #[cfg(feature = "ctx-send")]
        raw::Otpforward(
            self.c_ctx_ptr(),
            c_svc.as_ptr() as *mut c_char,
            data,
            len as c_long,
            flags as c_long,
        );
    }

    /// Forward a UBF request from the current service to another service.
    ///
    /// This consumes `data` and transfers ownership to Enduro/X. The function
    /// does not return to the caller in normal Enduro/X control flow.
    pub fn tpforward_ubf(&self, svc: &str, data: TypedUbf<'_>, flags: i64) {
        let ptr = data.into_raw();
        unsafe { self.tpforward(svc, ptr, 0, flags) };
    }

    pub(crate) unsafe fn tpexit(&self) {
        #[cfg(not(feature = "ctx-send"))]
        raw::tpexit();

        #[cfg(feature = "ctx-send")]
        raw::Otpexit(self.c_ctx_ptr());
    }

    pub(crate) unsafe fn tpcontinue(&self) {
        #[cfg(not(feature = "ctx-send"))]
        raw::tpcontinue();

        #[cfg(feature = "ctx-send")]
        raw::Otpcontinue(self.c_ctx_ptr());
    }

    pub(crate) unsafe fn tpsrvgetctxdata(&self) -> *mut c_char {
        #[cfg(not(feature = "ctx-send"))]
        {
            raw::tpsrvgetctxdata()
        }

        #[cfg(feature = "ctx-send")]
        {
            raw::Otpsrvgetctxdata(self.c_ctx_ptr())
        }
    }

    pub(crate) unsafe fn tpsrvgetctxdata2(
        &self,
        p_buf: *mut c_char,
        p_len: *mut c_long,
    ) -> *mut c_char {
        #[cfg(not(feature = "ctx-send"))]
        {
            raw::tpsrvgetctxdata2(p_buf, p_len)
        }

        #[cfg(feature = "ctx-send")]
        {
            raw::Otpsrvgetctxdata2(self.c_ctx_ptr(), p_buf, p_len)
        }
    }

    pub(crate) unsafe fn tpsrvfreectxdata(&self, p_buf: *mut c_char) {
        #[cfg(not(feature = "ctx-send"))]
        raw::tpsrvfreectxdata(p_buf);

        #[cfg(feature = "ctx-send")]
        raw::Otpsrvfreectxdata(self.c_ctx_ptr(), p_buf);
    }

    pub(crate) fn tpsrvsetctxdata(&self, data: *mut c_char, flags: i64) -> AtmiResult<()> {
        #[cfg(not(feature = "ctx-send"))]
        let rc = unsafe { raw::tpsrvsetctxdata(data, flags as c_long) };

        #[cfg(feature = "ctx-send")]
        let rc = unsafe { raw::Otpsrvsetctxdata(self.c_ctx_ptr(), data, flags as c_long) };

        self.rc_to_result(rc)
    }

    /// # Main-thread only
    ///
    /// Enduro/X keeps the poller extensions in a single global list
    /// (`ndrx_G_pollext`) and mirrors them into the shared `epollfd`, with no
    /// locking anywhere (`libatmisrv/pollextension.c`). The main dispatch
    /// thread walks that list on every poll iteration.
    ///
    /// Calling this from a service handler is therefore only safe while
    /// `maxdispatchthreads` is 1, where handlers run on the main thread. Under
    /// dispatch threading a handler runs on a worker, and mutating the list
    /// there races the poll loop -- which in practice wedges the server and
    /// makes it stop answering requests. Register extensions from a callback
    /// that already runs on the main poll thread instead.
    pub fn tpext_addpollerfd(
        &self,
        fd: i32,
        events: u32,
        user_data: usize,
        callback: RustPollerCallback,
    ) -> AtmiResult<()> {
        require_main_thread("tpext_addpollerfd")?;
        {
            let mut rt = extension_runtime().lock().map_err(|_| runtime_lock_err())?;
            rt.pollers.insert(fd, (callback, user_data));
        }

        #[cfg(not(feature = "ctx-send"))]
        let rc = unsafe {
            raw::tpext_addpollerfd(
                fd as c_int,
                events,
                std::ptr::null_mut(),
                Some(rust_poller_dispatch),
            )
        };

        #[cfg(feature = "ctx-send")]
        let rc = unsafe {
            raw::Otpext_addpollerfd(
                self.c_ctx_ptr(),
                fd as c_int,
                events,
                std::ptr::null_mut(),
                Some(rust_poller_dispatch),
            )
        };

        if rc == raw::EXSUCCEED as c_int {
            Ok(())
        } else {
            if let Ok(mut rt) = extension_runtime().lock() {
                rt.pollers.remove(&fd);
            }
            Err(self.atmi_last_error())
        }
    }

    /// # Main-thread only
    ///
    /// Enduro/X keeps the poller extensions in a single global list
    /// (`ndrx_G_pollext`) and mirrors them into the shared `epollfd`, with no
    /// locking anywhere (`libatmisrv/pollextension.c`). The main dispatch
    /// thread walks that list on every poll iteration.
    ///
    /// Calling this from a service handler is therefore only safe while
    /// `maxdispatchthreads` is 1, where handlers run on the main thread. Under
    /// dispatch threading a handler runs on a worker, and mutating the list
    /// there races the poll loop -- which in practice wedges the server and
    /// makes it stop answering requests. Register extensions from a callback
    /// that already runs on the main poll thread instead.
    pub fn tpext_delpollerfd(&self, fd: i32) -> AtmiResult<()> {
        require_main_thread("tpext_delpollerfd")?;
        #[cfg(not(feature = "ctx-send"))]
        let rc = unsafe { raw::tpext_delpollerfd(fd as c_int) };

        #[cfg(feature = "ctx-send")]
        let rc = unsafe { raw::Otpext_delpollerfd(self.c_ctx_ptr(), fd as c_int) };

        if rc == raw::EXSUCCEED as c_int {
            if let Ok(mut rt) = extension_runtime().lock() {
                rt.pollers.remove(&fd);
            }
            Ok(())
        } else {
            Err(self.atmi_last_error())
        }
    }

    /// # Main-thread only
    ///
    /// Enduro/X keeps the poller extensions in a single global list
    /// (`ndrx_G_pollext`) and mirrors them into the shared `epollfd`, with no
    /// locking anywhere (`libatmisrv/pollextension.c`). The main dispatch
    /// thread walks that list on every poll iteration.
    ///
    /// Calling this from a service handler is therefore only safe while
    /// `maxdispatchthreads` is 1, where handlers run on the main thread. Under
    /// dispatch threading a handler runs on a worker, and mutating the list
    /// there races the poll loop -- which in practice wedges the server and
    /// makes it stop answering requests. Register extensions from a callback
    /// that already runs on the main poll thread instead.
    pub fn tpext_addperiodcb(&self, secs: i32, callback: RustPeriodCallback) -> AtmiResult<()> {
        require_main_thread("tpext_addperiodcb")?;
        {
            let mut rt = extension_runtime().lock().map_err(|_| runtime_lock_err())?;
            rt.period_cb = Some(callback);
        }

        #[cfg(not(feature = "ctx-send"))]
        let rc = unsafe { raw::tpext_addperiodcb(secs as c_int, Some(rust_period_dispatch)) };

        #[cfg(feature = "ctx-send")]
        let rc = unsafe {
            raw::Otpext_addperiodcb(self.c_ctx_ptr(), secs as c_int, Some(rust_period_dispatch))
        };

        if rc == raw::EXSUCCEED as c_int {
            Ok(())
        } else {
            if let Ok(mut rt) = extension_runtime().lock() {
                rt.period_cb = None;
            }
            Err(self.atmi_last_error())
        }
    }

    /// # Main-thread only
    ///
    /// Enduro/X keeps the poller extensions in a single global list
    /// (`ndrx_G_pollext`) and mirrors them into the shared `epollfd`, with no
    /// locking anywhere (`libatmisrv/pollextension.c`). The main dispatch
    /// thread walks that list on every poll iteration.
    ///
    /// Calling this from a service handler is therefore only safe while
    /// `maxdispatchthreads` is 1, where handlers run on the main thread. Under
    /// dispatch threading a handler runs on a worker, and mutating the list
    /// there races the poll loop -- which in practice wedges the server and
    /// makes it stop answering requests. Register extensions from a callback
    /// that already runs on the main poll thread instead.
    pub fn tpext_delperiodcb(&self) -> AtmiResult<()> {
        require_main_thread("tpext_delperiodcb")?;
        #[cfg(not(feature = "ctx-send"))]
        let rc = unsafe { raw::tpext_delperiodcb() };

        #[cfg(feature = "ctx-send")]
        let rc = unsafe { raw::Otpext_delperiodcb(self.c_ctx_ptr()) };

        if rc == raw::EXSUCCEED as c_int {
            if let Ok(mut rt) = extension_runtime().lock() {
                rt.period_cb = None;
            }
            Ok(())
        } else {
            Err(self.atmi_last_error())
        }
    }

    /// # Main-thread only
    ///
    /// Enduro/X keeps the poller extensions in a single global list
    /// (`ndrx_G_pollext`) and mirrors them into the shared `epollfd`, with no
    /// locking anywhere (`libatmisrv/pollextension.c`). The main dispatch
    /// thread walks that list on every poll iteration.
    ///
    /// Calling this from a service handler is therefore only safe while
    /// `maxdispatchthreads` is 1, where handlers run on the main thread. Under
    /// dispatch threading a handler runs on a worker, and mutating the list
    /// there races the poll loop -- which in practice wedges the server and
    /// makes it stop answering requests. Register extensions from a callback
    /// that already runs on the main poll thread instead.
    pub fn tpext_addb4pollcb(&self, callback: RustBeforePollCallback) -> AtmiResult<()> {
        require_main_thread("tpext_addb4pollcb")?;
        {
            let mut rt = extension_runtime().lock().map_err(|_| runtime_lock_err())?;
            rt.before_poll_cb = Some(callback);
        }

        #[cfg(not(feature = "ctx-send"))]
        let rc = unsafe { raw::tpext_addb4pollcb(Some(rust_before_poll_dispatch)) };

        #[cfg(feature = "ctx-send")]
        let rc =
            unsafe { raw::Otpext_addb4pollcb(self.c_ctx_ptr(), Some(rust_before_poll_dispatch)) };

        if rc == raw::EXSUCCEED as c_int {
            Ok(())
        } else {
            if let Ok(mut rt) = extension_runtime().lock() {
                rt.before_poll_cb = None;
            }
            Err(self.atmi_last_error())
        }
    }

    /// # Main-thread only
    ///
    /// Enduro/X keeps the poller extensions in a single global list
    /// (`ndrx_G_pollext`) and mirrors them into the shared `epollfd`, with no
    /// locking anywhere (`libatmisrv/pollextension.c`). The main dispatch
    /// thread walks that list on every poll iteration.
    ///
    /// Calling this from a service handler is therefore only safe while
    /// `maxdispatchthreads` is 1, where handlers run on the main thread. Under
    /// dispatch threading a handler runs on a worker, and mutating the list
    /// there races the poll loop -- which in practice wedges the server and
    /// makes it stop answering requests. Register extensions from a callback
    /// that already runs on the main poll thread instead.
    pub fn tpext_delb4pollcb(&self) -> AtmiResult<()> {
        require_main_thread("tpext_delb4pollcb")?;
        #[cfg(not(feature = "ctx-send"))]
        let rc = unsafe { raw::tpext_delb4pollcb() };

        #[cfg(feature = "ctx-send")]
        let rc = unsafe { raw::Otpext_delb4pollcb(self.c_ctx_ptr()) };

        if rc == raw::EXSUCCEED as c_int {
            if let Ok(mut rt) = extension_runtime().lock() {
                rt.before_poll_cb = None;
            }
            Ok(())
        } else {
            Err(self.atmi_last_error())
        }
    }

    pub fn tpgetsrvid(&self) -> AtmiResult<i32> {
        #[cfg(not(feature = "ctx-send"))]
        let rc = unsafe { raw::tpgetsrvid() };

        #[cfg(feature = "ctx-send")]
        let rc = unsafe { raw::Otpgetsrvid(self.c_ctx_ptr()) };

        if rc == raw::EXFAIL as c_int {
            Err(self.atmi_last_error())
        } else {
            Ok(rc as i32)
        }
    }

    pub(crate) unsafe fn ndrx_main(&self, argc: i32, argv: *mut *mut c_char) -> i32 {
        #[cfg(not(feature = "ctx-send"))]
        {
            raw::ndrx_main(argc as c_int, argv)
        }

        #[cfg(feature = "ctx-send")]
        {
            raw::Ondrx_main(self.c_ctx_ptr(), argc as c_int, argv)
        }
    }

    pub(crate) unsafe fn ndrx_main_integra(
        &self,
        argc: i32,
        argv: *mut *mut c_char,
        in_tpsvrinit: Option<ServerInitHook>,
        in_tpsvrdone: Option<ServerDoneHook>,
        flags: i64,
    ) -> i32 {
        #[cfg(not(feature = "ctx-send"))]
        {
            raw::ndrx_main_integra(
                argc as c_int,
                argv,
                in_tpsvrinit,
                in_tpsvrdone,
                flags as c_long,
            )
        }

        #[cfg(feature = "ctx-send")]
        {
            raw::Ondrx_main_integra(
                self.c_ctx_ptr(),
                argc as c_int,
                argv,
                in_tpsvrinit,
                in_tpsvrdone,
                flags as c_long,
            )
        }
    }

    /// High-level server runner: hands the four C lifecycle hooks to Enduro/X
    /// and does not return until the server shuts down.
    ///
    /// Handles argv conversion and the low-level C callback wiring.
    ///
    /// ```no_run
    /// # use endurox_rs::{AtmiCtx, AtmiResult, ServerHooks};
    /// # fn my_init(_: &AtmiCtx, _: &[String]) -> AtmiResult<()> { Ok(()) }
    /// # fn my_done(_: &AtmiCtx) {}
    /// # fn my_thread_init(_: &AtmiCtx, _: &[String]) -> AtmiResult<()> { Ok(()) }
    /// # fn run(ctx: &AtmiCtx) -> AtmiResult<()> {
    /// ctx.tp_run(
    ///     ServerHooks::new(my_init)
    ///         .done(my_done)
    ///         .thread_init(my_thread_init),
    /// )
    /// # }
    /// ```
    pub fn tp_run(&self, hooks: ServerHooks) -> AtmiResult<()> {
        self.tp_run_inner(hooks)
    }

    fn tp_run_inner(&self, hooks: ServerHooks) -> AtmiResult<()> {
        let self_addr = self as *const AtmiCtx as usize;
        {
            let mut rt = server_runtime().lock().map_err(|_| runtime_lock_err())?;
            if rt.ctx_addr != 0 {
                return Err(AtmiError::new(
                    raw::TPEPROTO,
                    "a server runtime is already active in this process",
                ));
            }
            rt.reset();
            rt.ctx_addr = self_addr;
            rt.main_thread_id = Some(std::thread::current().id());
            rt.init_hook = Some(hooks.init);
            rt.done_hook = hooks.done;
            rt.thread_init_hook = hooks.thread_init;
            rt.thread_done_hook = hooks.thread_done;
        }

        let _runtime_guard = ServerRuntimeGuard;
        let _thread_mode_guard = unsafe { ServerThreadModeGuard::enable() };

        let args: Vec<String> = std::env::args().collect();
        let mut cargs: Vec<CString> = args
            .iter()
            .map(|s| {
                CString::new(s.as_str())
                    .map_err(|_| AtmiError::new(raw::TPEINVAL, "argv contains NUL byte"))
            })
            .collect::<Result<_, _>>()?;
        let mut argv: Vec<*mut c_char> = cargs
            .iter_mut()
            .map(|s| s.as_ptr() as *mut c_char)
            .collect();

        let rc = unsafe {
            self.ndrx_main_integra(
                argv.len() as c_int,
                argv.as_mut_ptr(),
                Some(rust_server_init),
                Some(rust_server_done),
                raw::ATMI_SRVLIB_NOLONGJUMP as i64,
            )
        };

        if rc == raw::EXSUCCEED as c_int {
            Ok(())
        } else {
            let init_error = server_runtime().lock().ok().and_then(|rt| {
                // Prefer the main init hook's error; fall back to the first
                // worker-thread failure, which is otherwise invisible.
                rt.init_error
                    .clone()
                    .or_else(|| rt.thread_init_error.clone())
            });
            Err(init_error.unwrap_or_else(|| AtmiError::new(raw::TPESYSTEM, "ATMI server failed")))
        }
    }

    /// Return a typed buffer from a service callback.
    ///
    /// Consumes `data` so its `Drop` is **not** called — ownership is
    /// transferred to the XATMI framework. The buffer's tracked `len()` is
    /// forwarded as the `tpreturn` length argument (relevant for CARRAY/STRING;
    /// ignored for self-describing buffer types).
    pub fn tpreturn(&self, status: TpReturnStatus, rcode: i64, data: TypedBuffer<'_>, flags: i64) {
        let len = data.len();
        let ptr = data.into_raw();
        unsafe { self.tpreturn_raw(status.to_raw(), rcode, ptr, len, flags) };
    }

    /// Return a UBF buffer from a service callback.
    ///
    /// Convenience wrapper over [`AtmiCtx::tpreturn`] for the common UBF case.
    pub fn tpreturn_ubf(&self, status: TpReturnStatus, rcode: i64, data: TypedUbf<'_>, flags: i64) {
        self.tpreturn(status, rcode, data.into_inner(), flags);
    }
}
