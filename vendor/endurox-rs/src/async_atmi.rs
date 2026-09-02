use crate::{raw, AtmiCtx, AtmiError, AtmiResult, TypedBuffer};
use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::ffi::c_char;
use std::fmt;
use std::future::{poll_fn, Future};
use std::io;
use std::ops::Deref;
use std::pin::pin;
use std::task::{Poll, Waker};
use std::time::Instant;

#[cfg(any(feature = "async-io", feature = "tokio"))]
use std::os::fd::{FromRawFd, OwnedFd, RawFd};

/// Runtime adapter used by [`AsyncAtmiCtx`] to await an Enduro/X reply fd.
///
/// Drivers are intentionally small: the runtime-neutral demux owns call
/// descriptors, reply routing, cancellation and timeouts. A driver only
/// registers a duplicated reply descriptor, waits for readability, manages its
/// runtime's readiness token, and provides a timer future.
pub trait AsyncReplyDriver: Sized {
    /// Runtime-specific readiness state retained until the nonblocking
    /// `tpgetrply` attempt has completed.
    type Readiness<'a>
    where
        Self: 'a;

    /// Register the Enduro/X-owned reply descriptor.
    ///
    /// Implementations must duplicate `reply_fd`; they must neither close it
    /// nor change flags on its shared open-file description.
    fn register(reply_fd: i32) -> io::Result<Self>;

    /// Wait until the registered reply descriptor may be readable.
    fn readable(&self) -> impl Future<Output = io::Result<Self::Readiness<'_>>> + '_;

    /// Clear a readiness indication after `tpgetrply(TPNOBLOCK)` has reported
    /// that the reply queue is empty.
    fn clear_readiness(&self, readiness: &mut Self::Readiness<'_>);

    /// Sleep until an absolute standard-library deadline.
    fn sleep_until(&self, deadline: Instant) -> impl Future<Output = ()> + '_;
}

// ---------------------------------------------------------------------------
// Reply demultiplexer
// ---------------------------------------------------------------------------

/// A reply buffer parked for a call descriptor, held as a raw ATMI pointer.
///
/// Slots cannot hold a [`TypedBuffer`]: that borrows the [`AtmiCtx`] which
/// [`AsyncAtmiCtx`] also owns, which would make the adapter self-referential.
/// [`ReplyDemux::release`] frees anything still parked.
struct ParkedBuf {
    ptr: *mut c_char,
    len: usize,
}

// SAFETY: a corollary of `unsafe impl Send for AtmiCtx` in atmictx.rs, which
// establishes that a `ctx-send` context may move between threads. A ParkedBuf
// is an owned ATMI allocation belonging to such a context and has no thread
// affinity of its own, so it moves with its context exactly as an owned
// `TypedBuffer` does. The demux is only reachable through `&AsyncAtmiCtx`,
// which stays !Sync, so two threads can never touch one concurrently.
//
// Gated on the same feature as the impl it rests on. Without it the raw pointer
// silently makes `AsyncAtmiCtx` !Send; with it unconditionally, this would claim
// more than atmictx.rs justifies. `async` implies `ctx-send` today, so the gate
// is documentation -- and a tripwire if the two are ever decoupled.
#[cfg(feature = "ctx-send")]
unsafe impl Send for ParkedBuf {}

impl ParkedBuf {
    const EMPTY: Self = Self {
        ptr: std::ptr::null_mut(),
        len: 0,
    };

    /// # Safety
    /// `ctx` must be the context that allocated `self.ptr`.
    unsafe fn free(self, ctx: &AtmiCtx) {
        if !self.ptr.is_null() {
            drop(unsafe { TypedBuffer::from_raw(ctx, self.ptr) });
        }
    }
}

enum Slot {
    /// Registered, reply not yet collected.
    Waiting {
        /// Waker of the future awaiting this descriptor, once it has parked.
        waker: Option<Waker>,
        /// Whether a specific future owns this descriptor.
        ///
        /// `tpcall` owns its descriptor outright, so its reply is never
        /// eligible for `TPGETANY`. A descriptor from [`AsyncAtmiCtx::tpacall`]
        /// is registered only to protect the number from reuse, and stays
        /// unclaimed until a `tpgetrply` names it -- otherwise `TPGETANY` could
        /// never collect a manually submitted call.
        claimed: bool,
    },
    /// A reply, or a per-descriptor error, has been routed here.
    Ready {
        outcome: AtmiResult<()>,
        buf: ParkedBuf,
        /// Whether a future was already waiting on this exact descriptor when
        /// the reply landed. `TPGETANY` must not steal those: they belong to a
        /// specific pending `tpcall`. Only unclaimed replies -- from the
        /// caller's own `tpacall` -- are eligible for "give me any reply".
        claimed: bool,
    },
}

/// Deregisters a `TPGETANY` waiter when the collection attempt ends.
struct AnyWaiterGuard<'a> {
    demux: &'a ReplyDemux,
    id: u64,
}

impl<'a> AnyWaiterGuard<'a> {
    fn new(demux: &'a ReplyDemux) -> Self {
        Self {
            id: demux.register_any_waiter(),
            demux,
        }
    }

    fn id(&self) -> u64 {
        self.id
    }
}

impl Drop for AnyWaiterGuard<'_> {
    fn drop(&mut self) {
        self.demux.deregister_any_waiter(self.id);
    }
}

/// A registered `TPGETANY` waiter.
#[derive(Default)]
struct AnyWaiter {
    waker: Option<Waker>,
    /// Queue-level failure delivered to this waiter specifically.
    error: Option<AtmiError>,
}

/// Which descriptor a waiter is after.
#[derive(Clone, Copy)]
enum Target {
    /// One specific call descriptor.
    One(i32),
    /// Whichever unclaimed reply arrives first (`TPGETANY`), identified by the
    /// waiter's registration id so a queue-level error reaches this waiter and
    /// not some later, unrelated one.
    Any(u64),
}

/// Routes replies from one shared reply queue to per-descriptor slots.
///
/// The drain uses `tpgetrply(TPGETANY)`, which is what makes this correct
/// rather than merely faster. Without `TPGETANY`, a `tpgetrply` for one
/// descriptor that meets another descriptor's reply moves that reply into
/// Enduro/X's in-memory queue (`ndrx_add_to_memq`). The message leaves the OS
/// queue for good, so the reply fd never signals for it again and whoever was
/// waiting on it stalls. With `TPGETANY` every reply is accepted and the `cd`
/// out-parameter identifies its owner, so nothing is buffered out of band.
#[derive(Default)]
struct ReplyDemux {
    slots: RefCell<HashMap<i32, Slot>>,
    /// Buffer lent to `tpgetrply` during a drain. Swapped with a slot's buffer
    /// on each routed reply, so routing costs a pointer move, not a copy.
    scratch: RefCell<Option<ParkedBuf>>,
    /// Buffers whose waiter disappeared, kept until `release` can free them.
    orphans: RefCell<Vec<ParkedBuf>>,
    /// Registered `TPGETANY` waiters, keyed by a per-waiter id.
    ///
    /// These own no descriptor slot, so a queue-level failure has nowhere
    /// per-descriptor to be recorded. Registering them individually lets an
    /// undirected error be delivered to every waiter that was present when it
    /// happened -- and only to those, so it cannot leak into a later unrelated
    /// call the way a single shared error slot would.
    any_waiters: RefCell<HashMap<u64, AnyWaiter>>,
    next_any_id: Cell<u64>,
    /// Absolute deadlines recorded at submission time, keyed by descriptor.
    /// Populated by `AsyncAtmiCtx::tpacall`; a later `tpgetrply` must use the
    /// deadline from when the call was *sent*, not from when collection began.
    deadlines: RefCell<HashMap<i32, Option<Instant>>>,
    /// Guards against re-entering a drain from within a drain.
    draining: Cell<bool>,
}

impl fmt::Debug for ReplyDemux {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ReplyDemux")
            .field("slots", &self.slots.borrow().len())
            .finish_non_exhaustive()
    }
}

impl ReplyDemux {
    /// Register a slot for a descriptor known to be free.
    ///
    /// Never displaces anything: [`AsyncAtmiCtx::submit`] refuses to proceed
    /// with a descriptor that is still occupied, so any existing entry here
    /// would be a bug rather than something to clean up.
    fn register_fresh(&self, cd: i32, claimed: bool) {
        let previous = self.slots.borrow_mut().insert(
            cd,
            Slot::Waiting {
                waker: None,
                claimed,
            },
        );
        debug_assert!(
            previous.is_none(),
            "descriptor {cd} was reused while its slot was still occupied"
        );
    }

    /// Whether anything is still tracked for `cd`.
    ///
    /// Any occupied slot makes the number unsafe to reuse, not just one holding
    /// an uncollected reply. A `Ready { claimed: true }` entry normally means
    /// the owning future has been *woken but has not resumed yet* -- not that
    /// it was abandoned. Overwriting that would free the response out from
    /// under it and leave two futures chasing the same descriptor.
    fn is_descriptor_busy(&self, cd: i32) -> bool {
        self.slots.borrow().contains_key(&cd)
    }

    /// Register a slot for a descriptor the caller owns, from its own
    /// `tpacall`.
    ///
    /// Unlike [`Self::register_fresh`] this preserves a reply that a drain has
    /// already parked here. With a manual `tpacall`, the reply can easily
    /// arrive before `tpgetrply` is first called, and dropping it would hang
    /// the caller forever.
    fn claim(&self, cd: i32) {
        match self.slots.borrow_mut().entry(cd) {
            std::collections::hash_map::Entry::Vacant(slot) => {
                slot.insert(Slot::Waiting {
                    waker: None,
                    claimed: true,
                });
            }
            std::collections::hash_map::Entry::Occupied(mut slot) => {
                // Naming the descriptor claims it: this caller is collecting it
                // by number, so TPGETANY must no longer take it. Any reply
                // already parked here is preserved.
                match slot.get_mut() {
                    Slot::Waiting { claimed, .. } => *claimed = true,
                    Slot::Ready { claimed, .. } => *claimed = true,
                }
            }
        }
    }

    fn record_deadline(&self, cd: i32, deadline: Option<Instant>) {
        self.deadlines.borrow_mut().insert(cd, deadline);
    }

    /// Deadline recorded when `cd` was submitted, if this adapter sent it.
    ///
    /// Read-only on purpose. A collection attempt may end without completing
    /// the call -- TPNOBLOCK reporting TPEBLOCK, a driver error, or the future
    /// being dropped and later resumed -- and the deadline has to survive all
    /// of those. It is dropped only by [`Self::forget_deadline`], once the
    /// descriptor has actually completed or been cancelled.
    fn peek_deadline(&self, cd: i32) -> Option<Option<Instant>> {
        self.deadlines.borrow().get(&cd).copied()
    }

    /// Earliest deadline among all tracked descriptors.
    ///
    /// `TPGETANY` takes whichever call replies first, so it has no single
    /// descriptor to draw a deadline from -- the input `cd` is ignored by
    /// Enduro/X in that mode. Waking at the soonest outstanding deadline lets
    /// `call_scan_tout` expire whichever call actually times out first.
    fn earliest_deadline(&self) -> Option<Instant> {
        self.deadlines.borrow().values().filter_map(|d| *d).min()
    }

    fn forget_deadline(&self, cd: i32) {
        self.deadlines.borrow_mut().remove(&cd);
    }

    fn is_ready(&self, cd: i32) -> bool {
        matches!(self.slots.borrow().get(&cd), Some(Slot::Ready { .. }))
    }

    /// A reply nobody is waiting on by descriptor, eligible for `TPGETANY`.
    fn any_unclaimed(&self) -> Option<i32> {
        self.slots
            .borrow()
            .iter()
            .find_map(|(cd, slot)| match slot {
                Slot::Ready { claimed: false, .. } => Some(*cd),
                _ => None,
            })
    }

    fn is_target_ready(&self, target: Target) -> bool {
        match target {
            Target::One(cd) => self.is_ready(cd),
            // The pending error counts as readiness. `fail_all_waiting` stores
            // it and calls the waker, but the woken `poll_fn` re-evaluates this
            // predicate: without the error term it would see nothing ready,
            // return Pending, and swallow the wake -- parking forever on a
            // queue that has already failed.
            Target::Any(id) => self.any_unclaimed().is_some() || self.has_any_error(id),
        }
    }

    fn has_any_error(&self, id: u64) -> bool {
        self.any_waiters
            .borrow()
            .get(&id)
            .is_some_and(|entry| entry.error.is_some())
    }

    fn park_waker(&self, target: Target, id: u64, waker: &Waker) {
        match target {
            Target::One(cd) => {
                if let Some(Slot::Waiting {
                    waker: slot_waker, ..
                }) = self.slots.borrow_mut().get_mut(&cd)
                {
                    match slot_waker {
                        Some(existing) if existing.will_wake(waker) => {}
                        other => *other = Some(waker.clone()),
                    }
                }
            }
            Target::Any(_) => {
                // Nothing per-descriptor to hang this on, so it is stored
                // against the waiter's own registration.
                if let Some(entry) = self.any_waiters.borrow_mut().get_mut(&id) {
                    match &mut entry.waker {
                        Some(existing) if existing.will_wake(waker) => {}
                        other => *other = Some(waker.clone()),
                    }
                }
            }
        }
    }

    fn wake_any_waiters(&self) {
        let wakers: Vec<Waker> = self
            .any_waiters
            .borrow_mut()
            .values_mut()
            .filter_map(|entry| entry.waker.take())
            .collect();
        for waker in wakers {
            waker.wake();
        }
    }

    /// Register a `TPGETANY` waiter and return its id.
    fn register_any_waiter(&self) -> u64 {
        let id = self.next_any_id.get().wrapping_add(1);
        self.next_any_id.set(id);
        self.any_waiters
            .borrow_mut()
            .insert(id, AnyWaiter::default());
        id
    }

    fn deregister_any_waiter(&self, id: u64) {
        self.any_waiters.borrow_mut().remove(&id);
    }

    /// Take the queue-level error delivered to this specific waiter, if any.
    fn take_any_error(&self, id: u64) -> Option<AtmiError> {
        self.any_waiters
            .borrow_mut()
            .get_mut(&id)
            .and_then(|entry| entry.error.take())
    }

    /// Remove a slot, returning any buffer parked in it so the caller can free
    /// it against the owning context.
    fn deregister(&self, cd: i32) -> Option<ParkedBuf> {
        match self.slots.borrow_mut().remove(&cd) {
            Some(Slot::Ready { buf, .. }) => Some(buf),
            _ => None,
        }
    }

    /// Take a completed reply, moving its buffer into `data`.
    ///
    /// `data`'s previous buffer is handed back as the next drain scratch, so
    /// neither an allocation nor a copy happens on the common path.
    ///
    /// `flags` is the caller's original flag set: `TPNOCHANGE` is checked here
    /// rather than during the drain, because the drain reads into a scratch
    /// buffer whose type has nothing to do with the caller's.
    fn take_ready(
        &self,
        target: Target,
        data: &mut TypedBuffer<'_>,
        flags: i64,
    ) -> Option<(i32, AtmiResult<()>)> {
        let cd = match target {
            Target::One(cd) if self.is_ready(cd) => cd,
            Target::One(_) => return None,
            Target::Any(_) => self.any_unclaimed()?,
        };
        let Some(Slot::Ready { outcome, buf, .. }) = self.slots.borrow_mut().remove(&cd) else {
            return None;
        };
        Some((cd, self.hand_over(outcome, buf, data, flags)))
    }

    /// Move a collected reply into the caller's buffer, applying `TPNOCHANGE`.
    fn hand_over(
        &self,
        outcome: AtmiResult<()>,
        buf: ParkedBuf,
        data: &mut TypedBuffer<'_>,
        flags: i64,
    ) -> AtmiResult<()> {
        if buf.ptr.is_null() {
            // A routed error that carried no buffer; leave `data` as it was.
            return outcome;
        }

        if flags & raw::TPNOCHANGE as i64 != 0 {
            // Enduro/X would have raised TPEOTYPE against the buffer handed to
            // tpgetrply. It saw the scratch, so re-check against the real one.
            let incoming = unsafe { TypedBuffer::borrowed_from_raw(data.ctx, buf.ptr) };
            // Compare subtype as well as type: for VIEW buffers Enduro/X
            // treats a subtype change as a TPNOCHANGE failure too
            // (libatmi/typed_view.c: `strcmp(outbufobj->subtype, p_hdr->vname)`).
            // Non-VIEW buffers carry an empty subtype, so this stays correct
            // for them without a special case.
            match (incoming.tptypes(), data.tptypes()) {
                (Ok(got), Ok(want))
                    if got.type_name != want.type_name || got.subtype != want.subtype =>
                {
                    let message = format!(
                        "TPNOCHANGE: receiver expects {}/{} but got {}/{} buffer",
                        want.type_name, want.subtype, got.type_name, got.subtype
                    );
                    self.stash(buf);
                    return Err(AtmiError::new(raw::TPEOTYPE, message));
                }
                _ => {}
            }
        }

        let previous = ParkedBuf {
            ptr: data.as_ptr(),
            len: data.len(),
        };
        data.replace_ptr(buf.ptr);
        data.set_len(buf.len);
        self.stash(previous);
        outcome
    }

    /// Keep a spare buffer for the next drain, or set it aside to be freed.
    fn stash(&self, buf: ParkedBuf) {
        if buf.ptr.is_null() {
            return;
        }
        let mut scratch = self.scratch.borrow_mut();
        if scratch.is_none() {
            *scratch = Some(buf);
        } else {
            drop(scratch);
            self.orphans.borrow_mut().push(buf);
        }
    }

    /// Drain every reply currently available and route each to its slot.
    ///
    /// Synchronous and free of await points, so `draining` only has to guard
    /// against re-entrancy, never against parallelism.
    fn drain(&self, ctx: &AtmiCtx, flags: i64) {
        if self.draining.replace(true) {
            return;
        }
        self.drain_inner(ctx, flags);
        self.draining.set(false);
    }

    fn drain_inner(&self, ctx: &AtmiCtx, _flags: i64) {
        // Neutral flag policy. A drain uses TPGETANY, so it collects replies
        // for descriptors belonging to *other* waiters; applying the calling
        // future's flags would impose one caller's semantics on everybody
        // else's replies. Only the two flags the demux itself requires are
        // passed. Per-call semantics are handled elsewhere: TPNOCHANGE on
        // handover in `take_ready`, TPNOTIME via the deadline, and flags that
        // cannot be applied after the fact are rejected up front by
        // `check_supported_flags`.
        let get_flags = raw::TPNOBLOCK as i64 | raw::TPGETANY as i64;

        loop {
            // No early return on an empty slot table: a TPGETANY caller has no
            // descriptor to register, so its very first drain legitimately runs
            // with nothing registered. Bailing out here made that wait hang.

            let mut buffer = match self.take_scratch(ctx) {
                Ok(buffer) => buffer,
                Err(err) => return self.fail_all_waiting(err),
            };

            // Enduro/X call descriptors are 1-based, so 0 reliably means "the
            // call failed before any reply was matched to a descriptor".
            let mut cd = 0i32;
            let outcome = ctx.tpgetrply(&mut cd, &mut buffer, get_flags);

            let len = buffer.len();
            let buf = ParkedBuf {
                ptr: buffer.into_raw(),
                len,
            };

            let blocked = matches!(&outcome, Err(err) if err.code == raw::TPEBLOCK);
            if blocked {
                // Queue empty. Keep the buffer for the next drain.
                self.stash(buf);
                return;
            }

            if cd > 0 {
                self.route(cd, outcome, buf);
                continue;
            }

            self.stash(buf);
            match outcome {
                // No descriptor to attribute this to (TPEOS, TPESYSTEM, ...).
                // The reply queue is not usable, so fail everyone waiting.
                Err(err) => return self.fail_all_waiting(err),
                // Success without a descriptor should not happen; drop the
                // reply rather than routing it to an arbitrary slot.
                Ok(()) => return,
            }
        }
    }

    fn take_scratch<'c>(&self, ctx: &'c AtmiCtx) -> AtmiResult<TypedBuffer<'c>> {
        if let Some(buf) = self.scratch.borrow_mut().take() {
            // SAFETY: the pointer came from a TypedBuffer allocated by this
            // same context and has not been freed since.
            let mut buffer = unsafe { TypedBuffer::from_raw(ctx, buf.ptr) };
            buffer.set_len(buf.len);
            return Ok(buffer);
        }
        // Type and size are provisional: `tpgetrply` reallocates and converts
        // the buffer to match whatever the service actually returned.
        ctx.tpalloc("CARRAY", "", 1024)
    }

    fn route(&self, cd: i32, outcome: AtmiResult<()>, buf: ParkedBuf) {
        let waker = {
            let mut slots = self.slots.borrow_mut();
            match slots.get_mut(&cd) {
                Some(slot) => {
                    let (waker, was_claimed) = match slot {
                        Slot::Waiting { waker, claimed } => (waker.take(), *claimed),
                        // A second reply for a descriptor that already has one
                        // should not happen; keep the first.
                        Slot::Ready { .. } => {
                            drop(slots);
                            self.stash(buf);
                            return;
                        }
                    };

                    // Carry the registration's claim state through: a tpcall
                    // descriptor stays claimed, a manual tpacall one stays
                    // collectable by TPGETANY until a tpgetrply names it.
                    *slot = Slot::Ready {
                        outcome,
                        buf,
                        claimed: was_claimed,
                    };
                    waker
                }
                None => {
                    // No slot yet. This is the manual `tpacall` pattern: the
                    // caller owns the descriptor and has not called `tpgetrply`
                    // for it yet, so its reply arrived first. Park it unclaimed
                    // rather than dropping it -- dropping would hang that call.
                    //
                    // This cannot accumulate garbage. Enduro/X itself discards
                    // replies whose descriptor is not CALL_WAITING_FOR_ANS
                    // (libatmi/tpcall.c, "Dropping incoming message"), so a
                    // reply for a cancelled or timed-out call never reaches
                    // here in the first place.
                    slots.insert(
                        cd,
                        Slot::Ready {
                            outcome,
                            buf,
                            claimed: false,
                        },
                    );
                    None
                }
            }
        };
        if let Some(waker) = waker {
            waker.wake();
        }
        // A TPGETANY waiter has no descriptor to be woken through.
        self.wake_any_waiters();
    }

    fn fail_all_waiting(&self, err: AtmiError) {
        // Deliver to each TPGETANY waiter registered right now. Every one of
        // them gets a copy -- a single shared error would satisfy only the
        // first -- and waiters registered later are unaffected, so a queue
        // failure cannot spuriously fail an unrelated future call.
        {
            let mut any = self.any_waiters.borrow_mut();
            for entry in any.values_mut() {
                entry.error = Some(err.clone());
            }
        }
        let mut wakers = Vec::new();
        {
            let mut slots = self.slots.borrow_mut();
            for slot in slots.values_mut() {
                if let Slot::Waiting { waker, .. } = slot {
                    wakers.extend(waker.take());
                    *slot = Slot::Ready {
                        outcome: Err(err.clone()),
                        buf: ParkedBuf::EMPTY,
                        claimed: true,
                    };
                }
            }
        }
        for waker in wakers {
            waker.wake();
        }
        self.wake_any_waiters();
    }

    /// Free every buffer still held. Must run before the context is dropped.
    ///
    /// # Safety
    /// `ctx` must be the context these buffers were allocated from.
    unsafe fn release(&self, ctx: &AtmiCtx) {
        let slots: Vec<Slot> = self.slots.borrow_mut().drain().map(|(_, v)| v).collect();
        for slot in slots {
            if let Slot::Ready { buf, .. } = slot {
                unsafe { buf.free(ctx) };
            }
        }
        if let Some(buf) = self.scratch.borrow_mut().take() {
            unsafe { buf.free(ctx) };
        }
        for buf in self.orphans.borrow_mut().drain(..) {
            unsafe { buf.free(ctx) };
        }
    }
}

// ---------------------------------------------------------------------------

/// An ATMI context with one explicitly selected asynchronous reply driver.
///
/// The adapter owns the context, which guarantees that the reply fd is
/// registered only once. It dereferences to [`AtmiCtx`] for allocation,
/// logging, `tpacall`, and APIs unrelated to reply collection. Its inherent
/// `tpcall`, `tpgetrply`, `tpcancel`, and `tpterm` methods provide the
/// async-aware variants of those operations.
///
/// Several calls may be in flight at once. Replies are demultiplexed by call
/// descriptor, so one reply wakes exactly the future waiting for it.
///
/// Timeouts come from `NDRX_TOUT` / `tptoutset` / `tpsblktime`, exactly as for
/// the blocking API -- there is no per-call timeout argument. For a one-off
/// override, call [`AtmiCtx::tpsblktime`] with `TPBLK_NEXT` before the call.
///
/// `AsyncAtmiCtx` remains `!Sync`, so its call futures are `!Send`. Use them on
/// one executor thread (for example, a Tokio current-thread runtime or a smol
/// executor).
#[derive(Debug)]
pub struct AsyncAtmiCtx<D> {
    // Drivers own a duplicated reply fd and must be dropped before the context
    // closes the original descriptor. Rust drops fields in declaration order.
    driver: D,
    demux: ReplyDemux,
    context: AtmiCtx,
}

impl<D: AsyncReplyDriver> AsyncAtmiCtx<D> {
    /// Attach `driver` type `D` to an initialized ATMI context.
    ///
    /// `context.tpinit()` must have completed so Enduro/X has opened its reply
    /// queue. On a non-pollable backend this returns `TPEINVAL`.
    pub fn new(context: AtmiCtx) -> AtmiResult<Self> {
        let reply_fd = context.reply_queue_fd()?;
        let driver = D::register(reply_fd).map_err(|err| {
            AtmiError::new(
                raw::TPEOS,
                format!("failed to register Enduro/X reply queue fd: {err}"),
            )
        })?;
        Ok(Self {
            driver,
            demux: ReplyDemux::default(),
            context,
        })
    }

    /// Whether the linked Enduro/X build exposes an EPOLL/KQUEUE reply fd.
    pub const SUPPORTED: bool = cfg!(endurox_pollable);

    /// Access the underlying synchronous context explicitly.
    pub fn context(&self) -> &AtmiCtx {
        &self.context
    }

    /// Remove the asynchronous driver and return the synchronous context.
    ///
    /// No call future may still borrow this adapter.
    pub fn into_inner(self) -> AtmiCtx {
        // `Self` has a Drop impl, so the fields cannot simply be destructured.
        let this = std::mem::ManuallyDrop::new(self);
        // SAFETY: each field is moved out exactly once and `this` is never
        // dropped, so no field is observed after the move.
        unsafe {
            let driver = std::ptr::read(&this.driver);
            let demux = std::ptr::read(&this.demux);
            let context = std::ptr::read(&this.context);
            drop(driver);
            demux.release(&context);
            context
        }
    }

    /// Submit a request and asynchronously wait for its reply.
    ///
    /// Dropping the returned future after submission cancels its Enduro/X
    /// descriptor.
    pub async fn tpcall(
        &self,
        svc: &str,
        idata: &TypedBuffer<'_>,
        odata: &mut TypedBuffer<'_>,
        flags: i64,
    ) -> AtmiResult<()> {
        Self::check_supported_flags(flags)?;
        Self::reject_tpnoreply(flags)?;
        // Read before tpacall: a TPBLK_NEXT setting is one-shot and the call
        // consumes it.
        let deadline = self.deadline_for(flags)?;
        let cd = self.submit(svc, idata, flags, deadline, true)?;
        let mut pending = AsyncPendingCall::new(self, cd);
        // Always Target::One here: a tpcall owns its descriptor, so it must not
        // pick up someone else's reply even if the caller passed TPGETANY.
        let result = match self
            .await_reply(Target::One(pending.cd), odata, flags, deadline, false)
            .await
        {
            Ok((_, outcome)) => outcome,
            Err(err) => Err(err),
        };
        if result.is_ok() {
            pending.complete();
        }
        result
    }

    /// Submit a request without waiting, recording its deadline.
    ///
    /// Prefer this over `AtmiCtx::tpacall` (reachable through `Deref`) when the
    /// reply will be collected with [`Self::tpgetrply`]. The Enduro/X call
    /// timeout starts when the request is *sent*, and a `TPBLK_NEXT` setting is
    /// consumed by that send. Reading `tpgblktime` later, at collection time,
    /// yields a fresh interval measured from the wrong instant and a value that
    /// no longer reflects the one-shot override -- cancelling the descriptor
    /// too late, or too early.
    pub fn tpacall(&self, svc: &str, idata: &TypedBuffer<'_>, flags: i64) -> AtmiResult<i32> {
        Self::check_supported_flags(flags)?;
        let deadline = self.deadline_for(flags)?;
        self.submit(svc, idata, flags, deadline, false)
    }

    /// Send a request and take ownership of the descriptor, refusing to reuse
    /// one whose slot is still occupied.
    ///
    /// Enduro/X frees a descriptor number as soon as the demux collects its
    /// reply, so the same number can come straight back from `tpacall` while
    /// the previous reply is still tracked here. That number is the only
    /// identifier `tpgetrply` and `TPGETANY` have, so proceeding would either
    /// attribute the old reply to the new call or destroy it.
    /// `claimed` says whether a specific future owns the descriptor. `tpcall`
    /// does; `tpacall` hands the descriptor back to the caller, so its reply
    /// must stay collectable by `TPGETANY` until a `tpgetrply` names it.
    fn submit(
        &self,
        svc: &str,
        idata: &TypedBuffer<'_>,
        flags: i64,
        deadline: Option<Instant>,
        claimed: bool,
    ) -> AtmiResult<i32> {
        let cd = self.context.tpacall(svc, idata, flags)?;

        if cd > 0 && self.demux.is_descriptor_busy(cd) {
            // Cancel through the *context*, not through `Self::tpcancel`: the
            // adapter version calls `release_slot`, which would delete and free
            // the very reply the caller is about to be told to collect, and
            // discard its deadline along with it. Only the newly submitted
            // Enduro/X generation is cancelled here; the old slot is untouched.
            let _ = self.context.tpcancel(cd);
            // ndrx_tpcancel runs its own per-cd tpgetrply, which can shift
            // another descriptor's reply into the in-memory queue, where the
            // reply fd will never signal it again. Recover those.
            self.demux.drain(&self.context, 0);
            self.demux.wake_any_waiters();
            return Err(AtmiError::new(
                raw::TPELIMIT,
                "call descriptor is still tracked by an earlier call; collect or \
                 cancel that one before issuing another request",
            ));
        }

        if cd > 0 {
            self.demux.register_fresh(cd, claimed);
            self.demux.record_deadline(cd, deadline);
        }
        Ok(cd)
    }

    /// Compatibility spelling for [`AsyncAtmiCtx::tpcall`].
    pub async fn tpcall_async(
        &self,
        svc: &str,
        idata: &TypedBuffer<'_>,
        odata: &mut TypedBuffer<'_>,
        flags: i64,
    ) -> AtmiResult<()> {
        self.tpcall(svc, idata, odata, flags).await
    }

    /// Asynchronously retrieve a caller-owned descriptor returned by
    /// [`AtmiCtx::tpacall`].
    ///
    /// Expiration of the Enduro/X call timeout cancels `cd`. Merely dropping
    /// this future does not, because descriptor ownership remains with the
    /// caller; the wait can be resumed or cancelled explicitly.
    pub async fn tpgetrply(
        &self,
        cd: &mut i32,
        data: &mut TypedBuffer<'_>,
        flags: i64,
    ) -> AtmiResult<()> {
        Self::check_supported_flags(flags)?;

        let any = flags & raw::TPGETANY as i64 != 0;

        // Prefer the deadline recorded when the descriptor was submitted
        // through `AsyncAtmiCtx::tpacall`. Recomputing it here would measure
        // from the wrong instant and would miss a TPBLK_NEXT already consumed
        // by the send. The fallback covers descriptors submitted through the
        // raw context, where the submission time is not knowable to us.
        let deadline = if any {
            match self.demux.earliest_deadline() {
                Some(deadline) => Some(deadline),
                None => self.deadline_for(flags)?,
            }
        } else {
            match self.demux.peek_deadline(*cd) {
                Some(recorded) => recorded,
                None => self.deadline_for(flags)?,
            }
        };

        // A TPGETANY waiter registers so a queue-level failure can be routed to
        // it; the guard deregisters on every exit path, including panics.
        let any_waiter = if any {
            Some(AnyWaiterGuard::new(&self.demux))
        } else {
            None
        };

        let target = if let Some(guard) = &any_waiter {
            // TPGETANY: take whichever reply lands first and report which one
            // it was through `cd`, as the blocking API does. Replies belonging
            // to a pending `tpcall` future are not eligible -- that future
            // asked for its descriptor by name and TPGETANY must not steal it.
            Target::Any(guard.id())
        } else {
            // `claim`, not `register_fresh`: a drain may already have parked
            // this descriptor's reply before the caller got here.
            self.demux.claim(*cd);
            Target::One(*cd)
        };

        let (replied_cd, outcome) = self
            .await_reply(target, data, flags, deadline, true)
            .await?;
        // The call is finished, whatever its outcome, so drop its deadline: a
        // reused descriptor number must not inherit it. Under TPGETANY this is
        // the descriptor that actually replied, not the one passed in.
        self.demux.forget_deadline(replied_cd);
        *cd = replied_cd;
        outcome
    }

    /// Whether the caller asked for a nonblocking collection.
    fn is_nonblocking(flags: i64) -> bool {
        flags & raw::TPNOBLOCK as i64 != 0
    }

    /// Compatibility spelling for [`AsyncAtmiCtx::tpgetrply`].
    pub async fn tpgetrply_async(
        &self,
        cd: &mut i32,
        data: &mut TypedBuffer<'_>,
        flags: i64,
    ) -> AtmiResult<()> {
        self.tpgetrply(cd, data, flags).await
    }

    /// Cancel an asynchronous call and release its slot.
    pub fn tpcancel(&self, cd: i32) -> AtmiResult<()> {
        self.release_slot(cd);
        let result = self.context.tpcancel(cd);
        // ndrx_tpcancel performs its own per-cd
        // `ndrx_tpgetrply(.., TPNOBLOCK|TPNOABORT, ..)` (libatmi/tpcall.c), which
        // is exactly the shape that moves another descriptor's reply into
        // Enduro/X's in-memory queue. That reply will never make the fd readable
        // again, so drain here to pick it up and wake whoever was waiting on it.
        self.demux.drain(&self.context, 0);
        self.demux.wake_any_waiters();
        result
    }

    /// Deregister the async driver, terminate the context, and consume `self`.
    pub fn tpterm(self) -> AtmiResult<()> {
        self.into_inner().tpterm()
    }

    /// Reject reply-collection flags the demux cannot honour per call.
    ///
    /// A drain uses `TPGETANY` and therefore collects replies for descriptors
    /// other than the one whose future triggered it. Flags that change how
    /// Enduro/X behaves *at collection time* -- `TPNOABORT`, `TPTRANSUSPEND` --
    /// cannot be applied to only one of those replies, and applying them to all
    /// would silently alter unrelated calls. Failing loudly beats guessing.
    fn check_supported_flags(flags: i64) -> AtmiResult<()> {
        const UNSUPPORTED: &[(i64, &str)] = &[
            (raw::TPNOABORT as i64, "TPNOABORT"),
            (raw::TPTRANSUSPEND as i64, "TPTRANSUSPEND"),
        ];
        for (bit, name) in UNSUPPORTED {
            if flags & *bit != 0 {
                return Err(AtmiError::new(
                    raw::TPEINVAL,
                    format!(
                        "{name} cannot be honoured per call by the async reply demux,                          because one drain collects replies for several descriptors;                          use the blocking API for this call"
                    ),
                ));
            }
        }
        Ok(())
    }

    /// Mirror the public `tpcall()`'s rejection of `TPNOREPLY`.
    ///
    /// `libatmi/atmi.c:330` fails with TPEINVAL and the message "TPNOREPLY
    /// cannot be used with tpcall()" before delegating to `ndrx_tpcall`. The
    /// internal helper does tolerate the flag -- that is how `tppost` reuses it
    /// -- but the public call surface does not, and this adapter mirrors the
    /// public surface. It also avoids waiting on the descriptor 0 that
    /// `tpacall` returns for TPNOREPLY, which would never be satisfied.
    fn reject_tpnoreply(flags: i64) -> AtmiResult<()> {
        if flags & raw::TPNOREPLY as i64 != 0 {
            return Err(AtmiError::new(
                raw::TPEINVAL,
                "TPNOREPLY cannot be used with tpcall()",
            ));
        }
        Ok(())
    }

    /// Deadline for this call, honouring `TPNOTIME`.
    ///
    /// With `TPNOTIME` Enduro/X disables its own call timeout, so imposing the
    /// `tpgblktime` deadline here would expire and cancel a descriptor the
    /// caller explicitly asked to wait on indefinitely.
    fn deadline_for(&self, flags: i64) -> AtmiResult<Option<Instant>> {
        if flags & raw::TPNOTIME as i64 != 0 {
            return Ok(None);
        }
        self.context.reply_deadline()
    }

    fn release_slot(&self, cd: i32) {
        // The descriptor is finished with, so its recorded deadline must go
        // too. Leaving it behind lets `earliest_deadline` keep returning a
        // cancelled call's -- typically short -- deadline, which would make a
        // later TPGETANY wake and report a premature TPETIME.
        self.demux.forget_deadline(cd);
        if let Some(buf) = self.demux.deregister(cd) {
            // SAFETY: allocated by this context, which is still alive.
            unsafe { buf.free(&self.context) };
        }
    }

    /// Returns the descriptor that replied alongside its outcome, so a
    /// `TPGETANY` caller learns which call completed.
    async fn await_reply(
        &self,
        target: Target,
        data: &mut TypedBuffer<'_>,
        flags: i64,
        deadline: Option<Instant>,
        cancel_on_timeout: bool,
    ) -> AtmiResult<(i32, AtmiResult<()>)> {
        loop {
            // A reply may already have been routed here by another future's
            // drain, before this future was ever polled.
            if let Some(ready) = self.demux.take_ready(target, data, flags) {
                return Ok(ready);
            }

            self.demux.drain(&self.context, flags);

            if let Some(ready) = self.demux.take_ready(target, data, flags) {
                return Ok(ready);
            }

            // A drain failure with no descriptor attached lands here. Slot
            // waiters get it through their slot; a TPGETANY waiter has no slot,
            // so it is delivered to that waiter's own registration.
            if let Target::Any(id) = target {
                if let Some(err) = self.demux.take_any_error(id) {
                    return Err(err);
                }
            }

            if Self::is_nonblocking(flags) {
                // TPNOBLOCK means "report emptiness, do not wait".
                return Err(AtmiError::new(
                    raw::TPEBLOCK,
                    "TPNOBLOCK was specified and no reply is available",
                ));
            }

            let (wake, readiness) = self.wait_for_wake(target, deadline).await?;
            match wake {
                Wake::Progress => {
                    // Drain first, then tell the runtime the queue came up
                    // empty. Clearing before the drain can drop a readiness
                    // edge that still had data behind it.
                    self.demux.drain(&self.context, flags);
                    if let Some(ready) = self.demux.take_ready(target, data, flags) {
                        return Ok(ready);
                    }
                    if let Some(mut token) = readiness {
                        self.driver.clear_readiness(&mut token);
                    }
                    continue;
                }
                Wake::Timeout => {
                    // Drain once more so Enduro/X's own `call_scan_tout` can
                    // expire the descriptor and report TPETIME with its
                    // bookkeeping intact, rather than synthesizing one blindly.
                    self.demux.drain(&self.context, flags);
                    if let Some(ready) = self.demux.take_ready(target, data, flags) {
                        return Ok(ready);
                    }
                    if let Target::One(cd) = target {
                        if cancel_on_timeout {
                            // Adapter-level cancel, not `context.tpcancel`:
                            // ndrx_tpcancel does its own per-cd tpgetrply and
                            // can push another descriptor's reply into the
                            // in-memory queue, which only our drain will
                            // recover. It releases the slot itself.
                            let _ = self.tpcancel(cd);
                        } else {
                            self.release_slot(cd);
                        }
                    }
                    return Err(AtmiError::new(raw::TPETIME, "async reply wait timed out"));
                }
            }
        }
    }

    /// Park until this descriptor's slot is filled, the reply fd signals, or
    /// the deadline passes.
    ///
    /// Any future woken by the fd drains on behalf of everyone, so a reply for
    /// a descriptor whose own future is parked still gets delivered.
    /// Returns the wake reason together with any runtime readiness token.
    ///
    /// The token is deliberately handed back rather than cleared here. Tokio's
    /// `AsyncFd` contract is that readiness may only be cleared once the I/O
    /// has actually reported "would block"; clearing it before the drain runs
    /// would, on an edge-triggered registration, discard a readiness edge while
    /// data was still queued and stall the waiter.
    #[allow(clippy::type_complexity)]
    async fn wait_for_wake(
        &self,
        target: Target,
        deadline: Option<Instant>,
    ) -> AtmiResult<(Wake, Option<D::Readiness<'_>>)> {
        let readable = self.driver.readable();
        let mut readable = pin!(readable);

        let mut readiness = None;
        let wake = match deadline {
            Some(deadline) => {
                let timer = self.driver.sleep_until(deadline);
                let mut timer = pin!(timer);
                poll_fn(|cx| {
                    match target {
                        Target::One(cd) => self.demux.park_waker(target, cd as u64, cx.waker()),
                        Target::Any(id) => self.demux.park_waker(target, id, cx.waker()),
                    }
                    if self.demux.is_target_ready(target) {
                        return Poll::Ready(Ok(Wake::Progress));
                    }
                    if let Poll::Ready(result) = readable.as_mut().poll(cx) {
                        return Poll::Ready(match result {
                            Ok(token) => {
                                readiness = Some(token);
                                Ok(Wake::Progress)
                            }
                            Err(err) => Err(driver_error(err)),
                        });
                    }
                    if timer.as_mut().poll(cx).is_ready() {
                        return Poll::Ready(Ok(Wake::Timeout));
                    }
                    Poll::Pending
                })
                .await
            }
            None => {
                poll_fn(|cx| {
                    match target {
                        Target::One(cd) => self.demux.park_waker(target, cd as u64, cx.waker()),
                        Target::Any(id) => self.demux.park_waker(target, id, cx.waker()),
                    }
                    if self.demux.is_target_ready(target) {
                        return Poll::Ready(Ok(Wake::Progress));
                    }
                    if let Poll::Ready(result) = readable.as_mut().poll(cx) {
                        return Poll::Ready(match result {
                            Ok(token) => {
                                readiness = Some(token);
                                Ok(Wake::Progress)
                            }
                            Err(err) => Err(driver_error(err)),
                        });
                    }
                    Poll::Pending
                })
                .await
            }
        }?;

        Ok((wake, readiness))
    }
}

enum Wake {
    Progress,
    Timeout,
}

impl<D> Deref for AsyncAtmiCtx<D> {
    type Target = AtmiCtx;

    fn deref(&self) -> &Self::Target {
        &self.context
    }
}

impl<D> AsRef<AtmiCtx> for AsyncAtmiCtx<D> {
    fn as_ref(&self) -> &AtmiCtx {
        &self.context
    }
}

impl<D> Drop for AsyncAtmiCtx<D> {
    fn drop(&mut self) {
        // SAFETY: `context` is still alive and owns every parked buffer.
        unsafe { self.demux.release(&self.context) };
    }
}

impl AtmiCtx {
    /// Convert this initialized context into an async context using driver `D`.
    #[cfg(feature = "async")]
    pub fn into_async<D: AsyncReplyDriver>(self) -> AtmiResult<AsyncAtmiCtx<D>> {
        AsyncAtmiCtx::new(self)
    }

    /// Whether this Enduro/X build exposes a pollable reply descriptor.
    #[cfg(feature = "async")]
    pub const ASYNC_SUPPORTED: bool = cfg!(endurox_pollable);
}

struct AsyncPendingCall<'ctx, D: AsyncReplyDriver> {
    context: &'ctx AsyncAtmiCtx<D>,
    cd: i32,
    armed: bool,
}

impl<'ctx, D: AsyncReplyDriver> AsyncPendingCall<'ctx, D> {
    fn new(context: &'ctx AsyncAtmiCtx<D>, cd: i32) -> Self {
        Self {
            context,
            cd,
            armed: true,
        }
    }

    fn complete(&mut self) {
        self.armed = false;
    }
}

impl<D: AsyncReplyDriver> Drop for AsyncPendingCall<'_, D> {
    fn drop(&mut self) {
        if self.armed {
            let _ = self.context.tpcancel(self.cd);
        } else {
            self.context.release_slot(self.cd);
        }
    }
}

fn driver_error(err: io::Error) -> AtmiError {
    AtmiError::new(
        raw::TPEOS,
        format!("async wait on Enduro/X reply queue failed: {err}"),
    )
}

#[cfg(any(feature = "async-io", feature = "tokio"))]
fn duplicate_reply_fd(reply_fd: RawFd) -> io::Result<OwnedFd> {
    let duplicate = unsafe { libc::fcntl(reply_fd, libc::F_DUPFD_CLOEXEC, 0) };
    if duplicate < 0 {
        Err(io::Error::last_os_error())
    } else {
        Ok(unsafe { OwnedFd::from_raw_fd(duplicate) })
    }
}

/// Tokio-native reply-fd driver.
#[cfg(feature = "tokio")]
#[derive(Debug)]
pub struct TokioReplyDriver {
    reply_fd: tokio::io::unix::AsyncFd<OwnedFd>,
    // The Enduro/X context is !Sync and its futures are local. Keeping the
    // driver local also prevents moving a reactor registration to a new runtime.
    _local: std::marker::PhantomData<std::rc::Rc<()>>,
}

#[cfg(feature = "tokio")]
impl AsyncReplyDriver for TokioReplyDriver {
    type Readiness<'a> = tokio::io::unix::AsyncFdReadyGuard<'a, OwnedFd>;

    fn register(reply_fd: i32) -> io::Result<Self> {
        let reply_fd = duplicate_reply_fd(reply_fd)?;
        let reply_fd = std::panic::catch_unwind(|| {
            tokio::io::unix::AsyncFd::with_interest(reply_fd, tokio::io::Interest::READABLE)
        })
        .map_err(|_| io::Error::other("Tokio runtime does not have an I/O driver enabled"))??;
        Ok(Self {
            reply_fd,
            _local: std::marker::PhantomData,
        })
    }

    fn readable(&self) -> impl Future<Output = io::Result<Self::Readiness<'_>>> + '_ {
        self.reply_fd.readable()
    }

    fn clear_readiness(&self, readiness: &mut Self::Readiness<'_>) {
        readiness.clear_ready();
    }

    fn sleep_until(&self, deadline: Instant) -> impl Future<Output = ()> + '_ {
        tokio::time::sleep_until(deadline.into())
    }
}

/// [`AsyncAtmiCtx`] using Tokio's native reactor.
#[cfg(feature = "tokio")]
pub type TokioAtmiCtx = AsyncAtmiCtx<TokioReplyDriver>;

#[cfg(feature = "tokio")]
impl AtmiCtx {
    /// Convert this initialized context to a Tokio-native async context.
    pub fn into_tokio(self) -> AtmiResult<TokioAtmiCtx> {
        self.into_async()
    }

    /// Compatibility capability flag for the Tokio adapter.
    pub const TOKIO_ASYNC_SUPPORTED: bool = cfg!(endurox_pollable);
}

/// Executor-independent driver backed by the `async-io` reactor.
#[cfg(feature = "async-io")]
#[derive(Debug)]
pub struct AsyncIoReplyDriver {
    reply_fd: async_io::Async<OwnedFd>,
}

#[cfg(feature = "async-io")]
impl AsyncReplyDriver for AsyncIoReplyDriver {
    type Readiness<'a> = ();

    fn register(reply_fd: i32) -> io::Result<Self> {
        let reply_fd = duplicate_reply_fd(reply_fd)?;
        Ok(Self {
            // Enduro/X controls O_NONBLOCK around tpgetrply. Do not mutate the
            // shared open-file-description flags from the duplicate.
            reply_fd: async_io::Async::new_nonblocking(reply_fd)?,
        })
    }

    fn readable(&self) -> impl Future<Output = io::Result<Self::Readiness<'_>>> + '_ {
        self.reply_fd.readable()
    }

    fn clear_readiness(&self, _readiness: &mut Self::Readiness<'_>) {
        // Each async-io Readable future consumes one reactor readiness tick.
    }

    #[allow(clippy::manual_async_fn)]
    fn sleep_until(&self, deadline: Instant) -> impl Future<Output = ()> + '_ {
        async move {
            let _ = async_io::Timer::at(deadline).await;
        }
    }
}

/// [`AsyncAtmiCtx`] using the executor-independent `async-io` reactor.
#[cfg(feature = "async-io")]
pub type AsyncIoAtmiCtx = AsyncAtmiCtx<AsyncIoReplyDriver>;

#[cfg(feature = "async-io")]
impl AtmiCtx {
    /// Convert this initialized context to an executor-independent async context.
    pub fn into_async_io(self) -> AtmiResult<AsyncIoAtmiCtx> {
        self.into_async()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;
    use std::task::Wake;

    #[cfg(any(feature = "async-io", feature = "tokio"))]
    use std::io::{Read, Write};
    #[cfg(any(feature = "async-io", feature = "tokio"))]
    use std::os::fd::AsRawFd;
    #[cfg(any(feature = "async-io", feature = "tokio"))]
    use std::os::unix::net::UnixStream;

    struct CountingWaker(AtomicUsize);

    impl Wake for CountingWaker {
        fn wake(self: Arc<Self>) {
            self.0.fetch_add(1, Ordering::SeqCst);
        }
        fn wake_by_ref(self: &Arc<Self>) {
            self.0.fetch_add(1, Ordering::SeqCst);
        }
    }

    /// The stranding scenario, exercised against the routing table alone so it
    /// runs on every backend, including non-pollable ones where the rest of
    /// the async path is compiled out.
    ///
    /// A reply for descriptor 7 is routed while 7's own future is parked. The
    /// slot must hold it and wake 7 -- not drop it, and not give it to 5.
    #[test]
    fn reply_is_routed_to_its_own_descriptor() {
        let demux = ReplyDemux::default();
        demux.register_fresh(5, true);
        demux.register_fresh(7, true);

        let counter = Arc::new(CountingWaker(AtomicUsize::new(0)));
        demux.park_waker(Target::One(7), 7, &Waker::from(counter.clone()));

        assert!(!demux.is_ready(7));
        demux.route(7, Ok(()), ParkedBuf::EMPTY);

        assert!(demux.is_ready(7), "reply must land in descriptor 7's slot");
        assert!(!demux.is_ready(5), "descriptor 5 must be untouched");
        assert_eq!(
            counter.0.load(Ordering::SeqCst),
            1,
            "the parked future for 7 must be woken exactly once"
        );
    }

    /// The manual `tpacall` pattern: the caller submits its own calls and
    /// collects them later. A reply that arrives before `tpgetrply` is called
    /// must be held, not dropped -- dropping it hangs that call forever.
    #[test]
    fn reply_arriving_before_tpgetrply_is_held_for_the_caller() {
        let demux = ReplyDemux::default();

        // Reply lands while the caller is still busy submitting others.
        demux.route(9, Ok(()), ParkedBuf::EMPTY);
        assert!(demux.is_ready(9), "unclaimed reply must be parked");

        // The caller now gets around to collecting descriptor 9.
        demux.claim(9);
        assert!(
            demux.is_ready(9),
            "claiming a descriptor must not discard a reply already parked for it"
        );
    }

    /// TPGETANY collects whichever unclaimed reply arrived, and reports which
    /// descriptor it belonged to.
    #[test]
    fn tpgetany_takes_any_unclaimed_reply() {
        let demux = ReplyDemux::default();

        let any_id = demux.register_any_waiter();
        assert!(demux.any_unclaimed().is_none(), "nothing to collect yet");

        // Replies from the caller's own tpacall calls, not yet collected.
        demux.route(4, Ok(()), ParkedBuf::EMPTY);
        assert!(demux.is_target_ready(Target::Any(any_id)));
        assert_eq!(demux.any_unclaimed(), Some(4));
    }

    /// The full manual path: `AsyncAtmiCtx::tpacall` registers the descriptor
    /// for reuse protection, its reply is routed into that registered slot, and
    /// `TPGETANY` must still be able to collect it.
    ///
    /// This is the case a bare `route()` without prior registration cannot
    /// catch: registering first is what makes `route()` take the "existing
    /// slot" branch, where the claim state has to be carried through rather
    /// than forced to true.
    #[test]
    fn tpgetany_collects_a_registered_manual_tpacall_reply() {
        let demux = ReplyDemux::default();
        let any_id = demux.register_any_waiter();

        // What AsyncAtmiCtx::tpacall does: registered, but unclaimed.
        demux.register_fresh(7, false);
        assert!(
            demux.is_descriptor_busy(7),
            "the descriptor is protected from reuse"
        );
        assert!(!demux.is_target_ready(Target::Any(any_id)));

        demux.route(7, Ok(()), ParkedBuf::EMPTY);

        assert_eq!(
            demux.any_unclaimed(),
            Some(7),
            "a manual tpacall reply must stay collectable by TPGETANY"
        );
        assert!(demux.is_target_ready(Target::Any(any_id)));
    }

    /// Naming a descriptor in `tpgetrply` claims it, so a concurrent TPGETANY
    /// stops being able to take that reply.
    #[test]
    fn naming_a_descriptor_claims_it_from_tpgetany() {
        let demux = ReplyDemux::default();
        let any_id = demux.register_any_waiter();

        demux.register_fresh(7, false);
        demux.claim(7);
        demux.route(7, Ok(()), ParkedBuf::EMPTY);

        assert!(demux.is_ready(7), "its own collector still finds it");
        assert!(
            !demux.is_target_ready(Target::Any(any_id)),
            "TPGETANY must not take a reply the caller asked for by number"
        );
    }

    /// Claiming after the reply has already been routed must preserve it.
    #[test]
    fn claiming_after_the_reply_landed_keeps_it() {
        let demux = ReplyDemux::default();

        demux.register_fresh(7, false);
        demux.route(7, Ok(()), ParkedBuf::EMPTY);
        demux.claim(7);

        assert!(
            demux.is_ready(7),
            "claiming a descriptor must not discard a reply already parked"
        );
    }

    /// TPGETANY must not steal a reply that a pending `tpcall` future is
    /// waiting for by descriptor -- that future would then hang forever.
    #[test]
    fn tpgetany_does_not_steal_a_claimed_reply() {
        let demux = ReplyDemux::default();

        let any_id = demux.register_any_waiter();
        // A tpcall future is awaiting descriptor 5.
        demux.register_fresh(5, true);
        demux.route(5, Ok(()), ParkedBuf::EMPTY);

        assert!(demux.is_ready(5), "the owner can still collect it");
        assert!(
            !demux.is_target_ready(Target::Any(any_id)),
            "TPGETANY must not see a reply owned by a pending tpcall"
        );

        // An unclaimed reply alongside it is still fair game.
        demux.route(6, Ok(()), ParkedBuf::EMPTY);
        assert_eq!(demux.any_unclaimed(), Some(6));
    }

    /// A TPGETANY waiter has no descriptor to be woken through, so every
    /// routed reply must wake it explicitly.
    #[test]
    fn tpgetany_waiter_is_woken_by_any_reply() {
        let demux = ReplyDemux::default();
        let any_id = demux.register_any_waiter();
        let counter = Arc::new(CountingWaker(AtomicUsize::new(0)));
        demux.park_waker(Target::Any(any_id), any_id, &Waker::from(counter.clone()));

        demux.route(3, Ok(()), ParkedBuf::EMPTY);

        assert_eq!(
            counter.0.load(Ordering::SeqCst),
            1,
            "a routed reply must wake the TPGETANY waiter"
        );
    }

    /// A queue-level failure must reach every TPGETANY waiter registered at the
    /// time, and must not leak into a waiter registered afterwards.
    #[test]
    fn undirected_error_reaches_each_current_any_waiter_only() {
        let demux = ReplyDemux::default();
        let first = demux.register_any_waiter();
        let second = demux.register_any_waiter();

        demux.fail_all_waiting(AtmiError::new(raw::TPEOS, "queue is gone"));

        assert!(
            demux.take_any_error(first).is_some(),
            "first TPGETANY waiter must receive the failure"
        );
        assert!(
            demux.take_any_error(second).is_some(),
            "second TPGETANY waiter must receive it too, not just the first"
        );

        // A waiter that arrives after the failure belongs to a later call and
        // must not inherit it.
        let later = demux.register_any_waiter();
        assert!(
            demux.take_any_error(later).is_none(),
            "a later waiter must not inherit an earlier queue failure"
        );
    }

    /// Any tracked descriptor is unsafe to reuse -- not merely one holding an
    /// *uncollected* reply.
    ///
    /// A claimed reply usually means the owning future has been woken and has
    /// not resumed yet. Treating that as reusable would free its response and
    /// leave two futures chasing the same descriptor number.
    #[test]
    fn any_tracked_descriptor_is_unsafe_to_reuse() {
        let demux = ReplyDemux::default();

        // Uncollected reply from a manual tpacall.
        demux.route(9, Ok(()), ParkedBuf::EMPTY);
        assert!(
            demux.is_descriptor_busy(9),
            "uncollected reply blocks reuse"
        );

        // Reply already routed to a waiting future that has not resumed yet.
        demux.register_fresh(4, true);
        demux.route(4, Ok(()), ParkedBuf::EMPTY);
        assert!(
            demux.is_descriptor_busy(4),
            "a claimed reply must block reuse too: its future may simply not              have resumed yet"
        );

        // A registered-but-unanswered descriptor is in use as well.
        demux.register_fresh(6, true);
        assert!(demux.is_descriptor_busy(6));

        // Only once the slot is gone does the number become reusable.
        drop(demux.deregister(9));
        assert!(!demux.is_descriptor_busy(9));
    }

    /// A queue error must make the waiter *ready*, not merely woken. Otherwise
    /// the re-polled future sees nothing ready, returns Pending, and the wake
    /// is swallowed.
    #[test]
    fn undirected_error_makes_the_any_waiter_ready() {
        let demux = ReplyDemux::default();
        let any_id = demux.register_any_waiter();

        assert!(!demux.is_target_ready(Target::Any(any_id)));
        demux.fail_all_waiting(AtmiError::new(raw::TPEOS, "queue is gone"));

        assert!(
            demux.is_target_ready(Target::Any(any_id)),
            "a pending queue error must count as readiness for its waiter"
        );
    }

    /// A descriptor released by its owner becomes reusable again, and
    /// registering it then starts from a clean slot.
    ///
    /// This is the counterpart to `any_tracked_descriptor_is_unsafe_to_reuse`:
    /// reuse is refused while the slot is occupied, and permitted once it is
    /// not. `register_fresh` therefore never has to displace anything.
    #[test]
    fn released_descriptor_becomes_reusable() {
        let demux = ReplyDemux::default();

        demux.route(9, Ok(()), ParkedBuf::EMPTY);
        assert!(demux.is_descriptor_busy(9));

        // Collected or cancelled by its owner.
        drop(demux.deregister(9));
        assert!(!demux.is_descriptor_busy(9));

        demux.register_fresh(9, true);
        assert!(
            !demux.is_ready(9),
            "the reused descriptor starts from a clean slot"
        );
        assert!(demux.is_descriptor_busy(9), "and is tracked again");
    }

    /// An error carrying no descriptor has to fail every waiter, because the
    /// reply queue itself is unusable at that point.
    #[test]
    fn undirected_error_fails_every_waiter() {
        let demux = ReplyDemux::default();
        demux.register_fresh(5, true);
        demux.register_fresh(7, true);

        demux.fail_all_waiting(AtmiError::new(raw::TPEOS, "queue is gone"));

        assert!(demux.is_ready(5));
        assert!(demux.is_ready(7));
    }

    /// Cancelling or timing out clears that waiter's slot without disturbing
    /// any other descriptor still in flight.
    ///
    /// Nothing should arrive for the cancelled one afterwards: Enduro/X drops
    /// replies whose descriptor is no longer CALL_WAITING_FOR_ANS. If one ever
    /// did, `route` parks it unclaimed and `release` reclaims the buffer, which
    /// is why this does not assert that a late reply is discarded.
    #[test]
    fn deregister_clears_only_that_descriptors_slot() {
        let demux = ReplyDemux::default();
        demux.register_fresh(5, true);
        demux.register_fresh(7, true);

        assert!(demux.deregister(5).is_none());
        assert!(!demux.is_ready(5), "cancelled descriptor keeps no slot");

        demux.route(7, Ok(()), ParkedBuf::EMPTY);
        assert!(
            demux.is_ready(7),
            "the live descriptor still routes normally"
        );
    }

    #[cfg(feature = "tokio")]
    #[test]
    fn tokio_driver_waits_without_owning_endurox_fd() {
        let (mut read_end, mut write_end) = UnixStream::pair().expect("socket pair failed");
        read_end
            .set_nonblocking(true)
            .expect("set_nonblocking failed");
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("Tokio runtime creation failed");
        let driver = runtime.block_on(async {
            TokioReplyDriver::register(read_end.as_raw_fd())
                .expect("Tokio driver registration failed")
        });

        write_end.write_all(b"x").expect("socket write failed");
        runtime.block_on(async {
            let mut readiness = driver.readable().await.expect("readiness wait failed");
            let mut byte = [0];
            read_end.read_exact(&mut byte).expect("socket read failed");
            assert_eq!(byte, *b"x");
            assert_eq!(
                read_end
                    .read(&mut byte)
                    .expect_err("empty socket should return WouldBlock")
                    .kind(),
                io::ErrorKind::WouldBlock
            );
            driver.clear_readiness(&mut readiness);
        });

        drop(driver);
        write_end
            .write_all(b"y")
            .expect("second socket write failed");
        let mut byte = [0];
        read_end
            .read_exact(&mut byte)
            .expect("original descriptor was closed by driver");
        assert_eq!(byte, *b"y");
    }

    #[cfg(feature = "async-io")]
    #[test]
    fn async_io_driver_waits_without_owning_endurox_fd() {
        let (mut read_end, mut write_end) = UnixStream::pair().expect("socket pair failed");
        read_end
            .set_nonblocking(true)
            .expect("set_nonblocking failed");
        let driver = AsyncIoReplyDriver::register(read_end.as_raw_fd())
            .expect("async-io driver registration failed");

        write_end.write_all(b"x").expect("socket write failed");
        async_io::block_on(async {
            // This driver's readiness token is `()`; async-io re-arms interest
            // per Readable future, so clear_readiness has nothing to carry.
            driver.readable().await.expect("readiness wait failed");
            let mut byte = [0];
            read_end.read_exact(&mut byte).expect("socket read failed");
            assert_eq!(byte, *b"x");
            driver.clear_readiness(&mut ());
        });

        drop(driver);
        write_end
            .write_all(b"y")
            .expect("second socket write failed");
        let mut byte = [0];
        read_end
            .read_exact(&mut byte)
            .expect("original descriptor was closed by driver");
        assert_eq!(byte, *b"y");
    }

    #[cfg(all(feature = "async-io", feature = "tokio"))]
    #[test]
    fn async_io_driver_future_runs_on_tokio_executor() {
        let (mut read_end, mut write_end) = UnixStream::pair().expect("socket pair failed");
        read_end
            .set_nonblocking(true)
            .expect("set_nonblocking failed");
        let driver = AsyncIoReplyDriver::register(read_end.as_raw_fd())
            .expect("async-io driver registration failed");
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("Tokio runtime creation failed");

        write_end.write_all(b"x").expect("socket write failed");
        runtime.block_on(async {
            driver
                .readable()
                .await
                .expect("Tokio did not poll async-io readiness");
            let mut byte = [0];
            read_end.read_exact(&mut byte).expect("socket read failed");
            assert_eq!(byte, *b"x");
        });
    }
}
