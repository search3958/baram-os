//! Small UEFI MP Services backed compute dispatcher.
//!
//! AP jobs must be pure CPU work: they must not call UEFI services or allocate.

use core::ffi::c_void;
use core::ptr;
use core::sync::atomic::{AtomicBool, AtomicPtr, AtomicUsize, Ordering};
use uefi::boot::{self, OpenProtocolAttributes, OpenProtocolParams};
use uefi::proto::pi::mp::MpServices;

type ErasedJob = unsafe fn(*const c_void, usize);

struct JobBatch {
    next: AtomicUsize,
    count: usize,
    data: *const c_void,
    run: ErasedJob,
}

unsafe impl Sync for JobBatch {}

static MP_SERVICES: AtomicPtr<MpServices> = AtomicPtr::new(ptr::null_mut());
static WORKER_COUNT: AtomicUsize = AtomicUsize::new(0);
static DISPATCHING: AtomicBool = AtomicBool::new(false);

/// Detect enabled processors and retain access to the firmware MP dispatcher.
/// Returns the number of AP workers available to compute jobs.
pub fn init() -> usize {
    // Raspberry Pi UEFI implementations commonly expose MP Services while
    // StartupAllAPs never completes. The first full-screen blur then appears
    // to freeze immediately after the boot logo. Keep pre-boot rendering on
    // the BSP on AArch64; it is deterministic and avoids a firmware deadlock.
    #[cfg(target_arch = "aarch64")]
    return 0;

    #[cfg(not(target_arch = "aarch64"))]
    {
    let Ok(handle) = boot::get_handle_for_protocol::<MpServices>() else {
        return 0;
    };
    let params = OpenProtocolParams {
        handle,
        agent: boot::image_handle(),
        controller: None,
    };
    let Ok(mp) =
        (unsafe { boot::open_protocol::<MpServices>(params, OpenProtocolAttributes::GetProtocol) })
    else {
        return 0;
    };
    let Ok(count) = mp.get_number_of_processors() else {
        return 0;
    };
    let workers = count.enabled.saturating_sub(1);
    if workers == 0 {
        return 0;
    }

    let protocol = (&*mp as *const MpServices).cast_mut();
    // MP Services remains installed throughout the UEFI boot-services phase.
    // Keep the GET_PROTOCOL open so the raw interface cannot be uninstalled.
    core::mem::forget(mp);
    MP_SERVICES.store(protocol, Ordering::Release);
    WORKER_COUNT.store(workers, Ordering::Release);
    workers
    }
}

pub fn worker_count() -> usize {
    WORKER_COUNT.load(Ordering::Acquire)
}

extern "efiapi" fn ap_worker(argument: *mut c_void) {
    let batch = unsafe { &*(argument as *const JobBatch) };
    loop {
        let index = batch.next.fetch_add(1, Ordering::Relaxed);
        if index >= batch.count {
            break;
        }
        unsafe { (batch.run)(batch.data, index) };
    }
}

/// Execute independent fixed-size jobs across all enabled APs. The caller's
/// data must not alias mutable output between job indices.
pub fn for_each<T: Sync>(count: usize, data: &T, job: fn(&T, usize)) {
    if count == 0 {
        return;
    }

    unsafe fn invoke<T>(erased: *const c_void, index: usize) {
        let invocation = &*(erased as *const Invocation<'_, T>);
        (invocation.job)(invocation.data, index);
    }

    struct Invocation<'a, T> {
        data: &'a T,
        job: fn(&T, usize),
    }

    let workers = worker_count();
    let mp_ptr = MP_SERVICES.load(Ordering::Acquire);
    // Avoid firmware dispatch overhead for tiny loops and prevent nested MP
    // calls when a worker-side rendering job invokes another parallel helper.
    if workers == 0
        || mp_ptr.is_null()
        || count < 2
        || DISPATCHING
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .is_err()
    {
        for index in 0..count {
            job(data, index);
        }
        return;
    }

    let invocation = Invocation { data, job };
    let batch = JobBatch {
        next: AtomicUsize::new(0),
        count,
        data: (&invocation as *const Invocation<'_, T>).cast(),
        run: invoke::<T>,
    };
    let result = unsafe {
        (&*mp_ptr).startup_all_aps(
            false,
            ap_worker,
            (&batch as *const JobBatch).cast_mut().cast(),
            None,
            None,
        )
    };
    DISPATCHING.store(false, Ordering::Release);

    // A firmware may temporarily reject dispatch if an AP is busy. Complete
    // only the unclaimed tail on the BSP; already claimed jobs have finished
    // before the blocking MP call returns.
    if result.is_err() {
        loop {
            let index = batch.next.fetch_add(1, Ordering::Relaxed);
            if index >= count {
                break;
            }
            job(data, index);
        }
    }
}
