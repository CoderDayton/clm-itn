use std::sync::OnceLock;

pub const GATE_NS_PER_TICK: f64 = 10.0;
pub const OVERHEAD_PAIRS: usize = 10_000;

pub fn ticks_to_ns(ticks: u64, ns_per_tick: f64) -> f64 {
    ticks as f64 * ns_per_tick
}

pub fn gate_decision(ns_per_tick: f64, invariant_tsc: bool) -> bool {
    invariant_tsc && ns_per_tick <= GATE_NS_PER_TICK
}

// Built under `test` on every host so the unit tests cover it.
#[cfg(any(test, target_arch = "x86_64"))]
#[cfg_attr(not(target_arch = "x86_64"), allow(dead_code))]
pub fn rate_from_leaf15(eax: u32, ebx: u32, ecx: u32) -> Option<f64> {
    if eax == 0 || ebx == 0 || ecx == 0 {
        return None;
    }
    Some(ecx as f64 * ebx as f64 / eax as f64)
}

#[cfg(any(test, target_arch = "aarch64"))]
#[cfg_attr(not(target_arch = "aarch64"), allow(dead_code))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Source {
    Generic,
    Pmu,
    Coarse,
}

#[cfg(any(test, target_arch = "aarch64"))]
#[cfg_attr(not(target_arch = "aarch64"), allow(dead_code))]
#[derive(Clone, Debug, PartialEq)]
pub struct Selection {
    pub source: Source,
    pub ns_per_tick: f64,
    pub note: Option<String>,
}

#[cfg(any(test, target_arch = "aarch64"))]
#[cfg_attr(not(target_arch = "aarch64"), allow(dead_code))]
pub fn select(
    generic_ns_per_tick: f64,
    try_pmu: impl FnOnce() -> Result<(f64, String), String>,
) -> Selection {
    if generic_ns_per_tick <= GATE_NS_PER_TICK {
        return Selection {
            source: Source::Generic,
            ns_per_tick: generic_ns_per_tick,
            note: None,
        };
    }
    match try_pmu() {
        Ok((ns_per_tick, detail)) => Selection {
            source: Source::Pmu,
            ns_per_tick,
            note: Some(format!(
                "PMU cycle counter ({detail}); cycles convert to ns only at a fixed clock, so lock this core's frequency"
            )),
        },
        Err(why) => Selection {
            source: Source::Coarse,
            ns_per_tick: generic_ns_per_tick,
            note: Some(format!(
                "coarse: generic counter is {generic_ns_per_tick:.2} ns/tick and the PMU is unavailable: {why}"
            )),
        },
    }
}

pub fn median(samples: &mut [u64]) -> u64 {
    samples.sort_unstable();
    samples[samples.len() / 2]
}

pub fn ns_per_tick() -> f64 {
    static NS_PER_TICK: OnceLock<f64> = OnceLock::new();
    *NS_PER_TICK.get_or_init(arch::ns_per_tick)
}

pub fn overhead_ticks() -> u64 {
    let mut samples = [0u64; OVERHEAD_PAIRS];
    for s in samples.iter_mut() {
        let t0 = start();
        let t1 = end();
        *s = t1.wrapping_sub(t0);
    }
    median(&mut samples)
}

pub fn gate_passes() -> bool {
    gate_decision(ns_per_tick(), invariant_tsc())
}

pub use arch::{backend, end, invariant_tsc, note, start};

static PMU_MISSES: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

pub fn pmu_misses() -> u64 {
    PMU_MISSES.load(std::sync::atomic::Ordering::Relaxed)
}

#[cfg(target_arch = "x86_64")]
fn calibrate() -> f64 {
    let t0 = std::time::Instant::now();
    let c0 = start();
    while t0.elapsed() < std::time::Duration::from_millis(100) {
        std::hint::spin_loop();
    }
    let c1 = end();
    let ns = t0.elapsed().as_nanos() as f64;
    ns / c1.wrapping_sub(c0) as f64
}

#[cfg(target_arch = "x86_64")]
mod arch {
    use core::arch::x86_64::{__cpuid, __rdtscp, _mm_lfence, _rdtsc};

    pub fn backend() -> &'static str {
        "x86_64 lfence;rdtsc / rdtscp;lfence"
    }

    pub fn note() -> Option<&'static str> {
        None
    }

    #[inline(always)]
    pub fn start() -> u64 {
        // SAFETY: lfence and rdtsc are baseline x86_64 instructions with no memory effects.
        unsafe {
            _mm_lfence();
            _rdtsc()
        }
    }

    #[inline(always)]
    pub fn end() -> u64 {
        let mut aux = 0u32;
        // SAFETY: rdtscp writes only `aux`; every x86_64 CPU with an invariant TSC has it.
        unsafe {
            let t = __rdtscp(&mut aux);
            _mm_lfence();
            t
        }
    }

    pub fn invariant_tsc() -> bool {
        __cpuid(0x8000_0000).eax >= 0x8000_0007 && __cpuid(0x8000_0007).edx & (1 << 8) != 0
    }

    pub fn ns_per_tick() -> f64 {
        let leaf15 = if __cpuid(0).eax >= 0x15 {
            let r = __cpuid(0x15);
            super::rate_from_leaf15(r.eax, r.ebx, r.ecx)
        } else {
            None
        };
        match leaf15 {
            Some(hz) => 1e9 / hz,
            None => super::calibrate(),
        }
    }
}

#[cfg(target_arch = "aarch64")]
mod arch {
    use super::{Selection, Source};
    use core::arch::asm;
    use std::sync::OnceLock;
    use std::time::{Duration, Instant};

    struct State {
        sel: Selection,
        #[cfg(target_os = "linux")]
        pmu: Option<crate::pmu::sys::Pmu>,
    }

    fn state() -> &'static State {
        static STATE: OnceLock<State> = OnceLock::new();
        STATE.get_or_init(init)
    }

    #[cfg(target_os = "linux")]
    fn init() -> State {
        let mut pmu = None;
        let sel = super::select(generic_ns_per_tick(), || {
            crate::pmu::sys::open().map(|(p, ns, detail)| {
                pmu = Some(p);
                (ns, detail)
            })
        });
        State { sel, pmu }
    }

    #[cfg(not(target_os = "linux"))]
    fn init() -> State {
        State {
            sel: super::select(generic_ns_per_tick(), || {
                Err("the PMU path needs Linux".into())
            }),
        }
    }

    #[inline(always)]
    fn read_cntvct() -> u64 {
        let t: u64;
        // SAFETY: cntvct_el0 is readable from EL0 on Linux and macOS; isb only orders execution.
        unsafe { asm!("isb", "mrs {t}, cntvct_el0", t = out(reg) t, options(nostack)) };
        t
    }

    #[inline(always)]
    fn read() -> u64 {
        #[cfg(target_os = "linux")]
        if let Some(p) = &state().pmu {
            let s = p.read();
            if !s.live {
                super::PMU_MISSES.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            }
            return s.count;
        }
        read_cntvct()
    }

    #[inline(always)]
    pub fn start() -> u64 {
        read()
    }

    #[inline(always)]
    pub fn end() -> u64 {
        read()
    }

    pub fn invariant_tsc() -> bool {
        true
    }

    pub fn backend() -> &'static str {
        match state().sel.source {
            Source::Generic => "aarch64 isb;mrs cntvct_el0",
            Source::Pmu => "aarch64 PMU cycle counter (perf user page; isb;mrs;isb)",
            Source::Coarse => "aarch64 isb;mrs cntvct_el0 (coarse)",
        }
    }

    pub fn note() -> Option<&'static str> {
        state().sel.note.as_deref()
    }

    pub fn ns_per_tick() -> f64 {
        state().sel.ns_per_tick
    }

    fn generic_ns_per_tick() -> f64 {
        let hz: u64;
        // SAFETY: cntfrq_el0 is readable from EL0 and has no side effects.
        unsafe { asm!("mrs {hz}, cntfrq_el0", hz = out(reg) hz, options(nomem, nostack)) };
        if hz != 0 {
            return 1e9 / hz as f64;
        }
        let t0 = Instant::now();
        let c0 = read_cntvct();
        while t0.elapsed() < Duration::from_millis(100) {
            std::hint::spin_loop();
        }
        t0.elapsed().as_nanos() as f64 / read_cntvct().wrapping_sub(c0).max(1) as f64
    }
}

#[cfg(not(any(target_arch = "x86_64", target_arch = "aarch64")))]
mod arch {
    use std::sync::OnceLock;
    use std::time::Instant;

    pub fn backend() -> &'static str {
        "std::time::Instant"
    }

    pub fn note() -> Option<&'static str> {
        None
    }

    fn epoch() -> Instant {
        static EPOCH: OnceLock<Instant> = OnceLock::new();
        *EPOCH.get_or_init(Instant::now)
    }

    #[inline(always)]
    pub fn start() -> u64 {
        epoch().elapsed().as_nanos() as u64
    }

    #[inline(always)]
    pub fn end() -> u64 {
        epoch().elapsed().as_nanos() as u64
    }

    pub fn invariant_tsc() -> bool {
        true
    }

    pub fn ns_per_tick() -> f64 {
        1.0
    }
}
