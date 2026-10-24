pub const ARMV8_PMUV3_CPU_CYCLES: u64 = 0x11;
#[cfg(test)]
pub const FLAG_DISABLED: u64 = 1 << 0;
pub const FLAG_EXCLUDE_KERNEL: u64 = 1 << 5;
pub const FLAG_EXCLUDE_HV: u64 = 1 << 6;
pub const CONFIG1_64BIT: u64 = 1 << 0;
pub const CONFIG1_USER_ACCESS: u64 = 1 << 1;
pub const CAP_USER_RDPMC: u64 = 1 << 2;

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PerfEventAttr {
    pub type_: u32,
    pub size: u32,
    pub config: u64,
    pub sample_period: u64,
    pub sample_type: u64,
    pub read_format: u64,
    pub flags: u64,
    pub wakeup_events: u32,
    pub bp_type: u32,
    pub config1: u64,
    pub config2: u64,
    pub branch_sample_type: u64,
    pub sample_regs_user: u64,
    pub sample_stack_user: u32,
    pub clockid: i32,
    pub sample_regs_intr: u64,
    pub aux_watermark: u32,
    pub sample_max_stack: u16,
    pub reserved_2: u16,
    pub aux_sample_size: u32,
    pub aux_action: u32,
    pub sig_data: u64,
    pub config3: u64,
}

impl PerfEventAttr {
    pub fn user_cycles(type_: u32, config: u64) -> Self {
        Self {
            type_,
            size: core::mem::size_of::<Self>() as u32,
            config,
            flags: FLAG_EXCLUDE_KERNEL | FLAG_EXCLUDE_HV,
            config1: CONFIG1_64BIT | CONFIG1_USER_ACCESS,
            ..Self::default()
        }
    }
}

#[repr(C)]
pub struct PageHead {
    pub version: u32,
    pub compat_version: u32,
    pub lock: u32,
    pub index: u32,
    pub offset: i64,
    pub time_enabled: u64,
    pub time_running: u64,
    pub capabilities: u64,
    pub pmc_width: u16,
}

pub trait UserPage {
    fn lock(&self) -> u32;
    fn index(&self) -> u32;
    fn offset(&self) -> i64;
    fn capabilities(&self) -> u64;
    fn pmc_width(&self) -> u16;
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Sample {
    pub count: u64,
    pub live: bool,
}

pub fn sign_extend(raw: u64, width: u16) -> i64 {
    if width == 0 || width >= 64 {
        return raw as i64;
    }
    let shift = 64 - u32::from(width);
    ((raw << shift) as i64) >> shift
}

pub fn read_count(page: &impl UserPage, mut rdpmc: impl FnMut(u32) -> u64) -> Sample {
    use core::sync::atomic::{Ordering, compiler_fence};
    loop {
        let seq = page.lock();
        compiler_fence(Ordering::SeqCst);
        let index = page.index();
        let offset = page.offset();
        let live = page.capabilities() & CAP_USER_RDPMC != 0 && index != 0;
        let pmc = if live {
            sign_extend(rdpmc(index - 1), page.pmc_width())
        } else {
            0
        };
        compiler_fence(Ordering::SeqCst);
        if page.lock() == seq {
            return Sample {
                count: offset.wrapping_add(pmc) as u64,
                live,
            };
        }
    }
}

pub fn parse_khz(s: &str) -> Option<f64> {
    match s.trim().parse::<u64>() {
        Ok(khz) if khz > 0 => Some(khz as f64 * 1e3),
        _ => None,
    }
}

pub fn parse_flag(s: &str) -> Option<bool> {
    s.trim().parse::<u64>().ok().map(|v| v != 0)
}

pub fn cpu_in_list(list: &str, cpu: u32) -> bool {
    list.trim().split(',').any(|part| {
        let mut ends = part.splitn(2, '-').map(|n| n.trim().parse::<u32>());
        match (ends.next(), ends.next()) {
            (Some(Ok(a)), None) => a == cpu,
            (Some(Ok(a)), Some(Ok(b))) => (a..=b).contains(&cpu),
            _ => false,
        }
    })
}

#[cfg(all(target_arch = "aarch64", target_os = "linux"))]
pub mod sys {
    use super::{
        ARMV8_PMUV3_CPU_CYCLES, CAP_USER_RDPMC, PageHead, PerfEventAttr, Sample, UserPage,
        cpu_in_list, parse_flag, parse_khz, read_count,
    };
    use core::arch::asm;
    use core::ptr::addr_of;
    use std::time::{Duration, Instant};

    const PERF_TYPE_HARDWARE: u32 = 0;
    const PERF_COUNT_HW_CPU_CYCLES: u64 = 0;
    const SYS_CLOSE: u64 = 57;
    const SYS_GETCPU: u64 = 168;
    const SYS_MUNMAP: u64 = 215;
    const SYS_MMAP: u64 = 222;
    const SYS_PERF_EVENT_OPEN: u64 = 241;
    const PROT_READ: u64 = 1;
    const MAP_SHARED: u64 = 1;
    const PERF_FLAG_FD_CLOEXEC: u64 = 1 << 3;
    const MAP_LEN: u64 = 4096;
    const NEG_ONE: u64 = u64::MAX;
    const CALIBRATION: Duration = Duration::from_millis(100);
    const USER_ACCESS: &str = "/proc/sys/kernel/perf_user_access";
    const PMU_DIR: &str = "/sys/bus/event_source/devices";

    unsafe fn syscall(nr: u64, args: [u64; 6]) -> i64 {
        let ret: i64;
        // SAFETY: upheld by the caller; svc only clobbers x0 here.
        unsafe {
            asm!(
                "svc #0",
                in("x8") nr,
                inlateout("x0") args[0] => ret,
                in("x1") args[1],
                in("x2") args[2],
                in("x3") args[3],
                in("x4") args[4],
                in("x5") args[5],
                options(nostack),
            );
        }
        ret
    }

    fn failed(ret: i64) -> bool {
        (-4095..0).contains(&ret)
    }

    fn errno(ret: i64) -> String {
        let name = match -ret {
            1 => " EPERM",
            2 => " ENOENT",
            13 => " EACCES",
            19 => " ENODEV",
            22 => " EINVAL",
            95 => " EOPNOTSUPP",
            _ => "",
        };
        format!("errno {}{name}", -ret)
    }

    fn close(fd: i64) {
        // SAFETY: closes a descriptor this module opened and no longer uses.
        unsafe { syscall(SYS_CLOSE, [fd as u64, 0, 0, 0, 0, 0]) };
    }

    fn current_cpu() -> Option<u32> {
        let mut cpu = 0u32;
        // SAFETY: getcpu writes one u32 through the first pointer; the other two are null.
        let ret = unsafe { syscall(SYS_GETCPU, [&raw mut cpu as u64, 0, 0, 0, 0, 0]) };
        (ret == 0).then_some(cpu)
    }

    macro_rules! read_pmu_counter {
        ($n:expr; $($i:literal)*) => {{
            let v: u64;
            match $n {
                $(
                    // SAFETY: the kernel enables EL0 reads of the counter the perf page indexes.
                    $i => unsafe {
                        asm!(
                            "isb",
                            concat!("mrs {v}, pmevcntr", stringify!($i), "_el0"),
                            "isb",
                            v = out(reg) v,
                            options(nostack),
                        )
                    },
                )*
                // SAFETY: as above; perf index 32 names the cycle counter.
                31 => unsafe {
                    asm!("isb", "mrs {v}, pmccntr_el0", "isb", v = out(reg) v, options(nostack))
                },
                _ => v = 0,
            }
            v
        }};
    }

    #[inline(always)]
    fn read_counter(n: u32) -> u64 {
        read_pmu_counter!(n; 0 1 2 3 4 5 6 7 8 9 10 11 12 13 14 15 16 17 18 19 20 21 22 23 24 25 26 27 28 29 30)
    }

    pub struct Pmu {
        head: *const PageHead,
    }

    // SAFETY: `head` points at a read-only shared mapping that lives until exit and is only read with volatile loads.
    unsafe impl Send for Pmu {}
    // SAFETY: as above.
    unsafe impl Sync for Pmu {}

    impl UserPage for Pmu {
        fn lock(&self) -> u32 {
            // SAFETY: `head` is the live perf mmap page.
            unsafe { addr_of!((*self.head).lock).read_volatile() }
        }
        fn index(&self) -> u32 {
            // SAFETY: as above.
            unsafe { addr_of!((*self.head).index).read_volatile() }
        }
        fn offset(&self) -> i64 {
            // SAFETY: as above.
            unsafe { addr_of!((*self.head).offset).read_volatile() }
        }
        fn capabilities(&self) -> u64 {
            // SAFETY: as above.
            unsafe { addr_of!((*self.head).capabilities).read_volatile() }
        }
        fn pmc_width(&self) -> u16 {
            // SAFETY: as above.
            unsafe { addr_of!((*self.head).pmc_width).read_volatile() }
        }
    }

    impl Pmu {
        #[inline(always)]
        pub fn read(&self) -> Sample {
            read_count(self, read_counter)
        }
    }

    pub fn open() -> Result<(Pmu, f64, String), String> {
        let access = std::fs::read_to_string(USER_ACCESS)
            .map_err(|_| format!("no {USER_ACCESS} (needs Linux 5.18+)"))?;
        if parse_flag(&access) != Some(true) {
            return Err(format!(
                "perf_user_access=0 (enable with: echo 1 > {USER_ACCESS})"
            ));
        }
        let cpu = current_cpu().ok_or("getcpu failed")?;
        let mut tries = vec![(
            "generic cycles".to_string(),
            PERF_TYPE_HARDWARE,
            PERF_COUNT_HW_CPU_CYCLES,
        )];
        tries.extend(pmu_types_for(cpu));
        let mut why = Vec::new();
        for (name, ty, config) in tries {
            match open_one(ty, config) {
                Ok(pmu) => {
                    let (ns, rate) = ns_per_cycle(&pmu, cpu);
                    return Ok((pmu, ns, format!("{name} on cpu{cpu}, {rate}")));
                }
                Err(e) => why.push(format!("{name}: {e}")),
            }
        }
        Err(why.join("; "))
    }

    fn pmu_types_for(cpu: u32) -> Vec<(String, u32, u64)> {
        let Ok(dir) = std::fs::read_dir(PMU_DIR) else {
            return Vec::new();
        };
        let mut out = Vec::new();
        for entry in dir.flatten() {
            let path = entry.path();
            let Ok(cpus) = std::fs::read_to_string(path.join("cpus")) else {
                continue;
            };
            let ty = std::fs::read_to_string(path.join("type"))
                .ok()
                .and_then(|t| t.trim().parse::<u32>().ok());
            if let (true, Some(ty)) = (cpu_in_list(&cpus, cpu), ty) {
                let name = entry.file_name().to_string_lossy().into_owned();
                out.push((name, ty, ARMV8_PMUV3_CPU_CYCLES));
            }
        }
        out
    }

    fn open_one(ty: u32, config: u64) -> Result<Pmu, String> {
        let attr = PerfEventAttr::user_cycles(ty, config);
        let attr_ptr = &raw const attr as u64;
        let args = [attr_ptr, 0, NEG_ONE, NEG_ONE, PERF_FLAG_FD_CLOEXEC, 0];
        // SAFETY: `attr` is a valid perf_event_attr whose size field equals its length; pid 0, cpu -1 counts this thread.
        let fd = unsafe { syscall(SYS_PERF_EVENT_OPEN, args) };
        if failed(fd) {
            return Err(format!("perf_event_open {}", errno(fd)));
        }
        let args = [0, MAP_LEN, PROT_READ, MAP_SHARED, fd as u64, 0];
        // SAFETY: maps the perf user page of `fd` read-only at a kernel-chosen address.
        let addr = unsafe { syscall(SYS_MMAP, args) };
        if failed(addr) {
            close(fd);
            return Err(format!("mmap {}", errno(addr)));
        }
        let pmu = Pmu {
            head: addr as usize as *const PageHead,
        };
        if pmu.read().live {
            return Ok(pmu);
        }
        let why = if pmu.capabilities() & CAP_USER_RDPMC == 0 {
            "cap_user_rdpmc clear"
        } else {
            "counter index 0 (pin to one core type)"
        };
        // SAFETY: unmaps the mapping created above; `pmu` is dropped right after.
        unsafe { syscall(SYS_MUNMAP, [addr as u64, MAP_LEN, 0, 0, 0, 0]) };
        close(fd);
        Err(why.into())
    }

    fn ns_per_cycle(pmu: &Pmu, cpu: u32) -> (f64, String) {
        let path = format!("/sys/devices/system/cpu/cpu{cpu}/cpufreq/cpuinfo_max_freq");
        let max = std::fs::read_to_string(&path).ok();
        if let Some(hz) = max.as_deref().and_then(parse_khz) {
            let mhz = hz / 1e6;
            return (1e9 / hz, format!("{mhz:.0} MHz from cpuinfo_max_freq"));
        }
        let t0 = Instant::now();
        let c0 = pmu.read().count;
        while t0.elapsed() < CALIBRATION {
            std::hint::spin_loop();
        }
        let c1 = pmu.read().count;
        let ns = t0.elapsed().as_nanos() as f64 / c1.wrapping_sub(c0).max(1) as f64;
        (ns, format!("{:.0} MHz calibrated over 100 ms", 1e3 / ns))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::cell::{Cell, RefCell};
    use core::mem::{offset_of, size_of};

    #[test]
    fn attr_matches_kernel_ver8_layout() {
        assert_eq!(size_of::<PerfEventAttr>(), 136);
        assert_eq!(offset_of!(PerfEventAttr, config), 8);
        assert_eq!(offset_of!(PerfEventAttr, flags), 40);
        assert_eq!(offset_of!(PerfEventAttr, wakeup_events), 48);
        assert_eq!(offset_of!(PerfEventAttr, config1), 56);
        assert_eq!(offset_of!(PerfEventAttr, config2), 64);
        assert_eq!(offset_of!(PerfEventAttr, sample_regs_intr), 96);
        assert_eq!(offset_of!(PerfEventAttr, aux_sample_size), 112);
        assert_eq!(offset_of!(PerfEventAttr, sig_data), 120);
        assert_eq!(offset_of!(PerfEventAttr, config3), 128);
    }

    #[test]
    fn user_cycles_attr_sets_size_flags_and_config1() {
        let a = PerfEventAttr::user_cycles(9, ARMV8_PMUV3_CPU_CYCLES);
        assert_eq!(a.size as usize, size_of::<PerfEventAttr>());
        assert_eq!((a.type_, a.config), (9, 0x11));
        assert_eq!(a.flags & FLAG_DISABLED, 0, "event must start enabled");
        assert_eq!(a.flags, (1 << 5) | (1 << 6));
        assert_eq!(a.config1, 0b11);
        let rest = PerfEventAttr {
            type_: 0,
            size: 0,
            config: 0,
            flags: 0,
            config1: 0,
            ..a
        };
        assert_eq!(rest, PerfEventAttr::default());
    }

    #[test]
    fn page_head_matches_kernel_offsets() {
        assert_eq!(offset_of!(PageHead, lock), 8);
        assert_eq!(offset_of!(PageHead, index), 12);
        assert_eq!(offset_of!(PageHead, offset), 16);
        assert_eq!(offset_of!(PageHead, capabilities), 40);
        assert_eq!(offset_of!(PageHead, pmc_width), 48);
    }

    #[test]
    fn sign_extend_uses_pmc_width() {
        assert_eq!(sign_extend(0xFFFF_FFFF_FFFF, 48), -1);
        assert_eq!(sign_extend(0x7FFF_FFFF_FFFF, 48), 0x7FFF_FFFF_FFFF);
        assert_eq!(sign_extend(0xDEAD_0000_0005, 32), 5);
        assert_eq!(sign_extend(u64::MAX, 64), -1);
        assert_eq!(sign_extend(42, 0), 42);
    }

    struct FakePage {
        locks: RefCell<Vec<u32>>,
        index: Cell<u32>,
        offset: i64,
        caps: u64,
        width: u16,
    }

    impl FakePage {
        fn new(locks: &[u32], index: u32) -> Self {
            Self {
                locks: RefCell::new(locks.iter().rev().copied().collect()),
                index: Cell::new(index),
                offset: 1_000,
                caps: CAP_USER_RDPMC | 0b10,
                width: 64,
            }
        }
    }

    impl UserPage for FakePage {
        fn lock(&self) -> u32 {
            let next = self.locks.borrow_mut().pop();
            next.expect("script ran out of lock reads")
        }
        fn index(&self) -> u32 {
            self.index.get()
        }
        fn offset(&self) -> i64 {
            self.offset
        }
        fn capabilities(&self) -> u64 {
            self.caps
        }
        fn pmc_width(&self) -> u16 {
            self.width
        }
    }

    fn live(count: u64) -> Sample {
        Sample { count, live: true }
    }

    fn dead(count: u64) -> Sample {
        Sample { count, live: false }
    }

    #[test]
    fn read_retries_until_lock_is_stable() {
        let page = FakePage::new(&[1, 2, 2, 2], 32);
        let mut calls = Vec::new();
        let s = read_count(&page, |n| {
            calls.push(n);
            500 + calls.len() as u64
        });
        assert_eq!(calls, [31, 31], "index 32 reads counter 31 (PMCCNTR)");
        assert_eq!(s, live(1_502));
        assert!(page.locks.borrow().is_empty());
    }

    #[test]
    fn read_adds_sign_extended_counter_to_offset() {
        let mut page = FakePage::new(&[4, 4], 3);
        page.width = 48;
        let s = read_count(&page, |n| {
            assert_eq!(n, 2);
            0xFFFF_FFFF_FFFF
        });
        assert_eq!(s, live(999));
    }

    #[test]
    fn read_without_user_counter_is_not_live() {
        let mut page = FakePage::new(&[6, 6], 3);
        page.caps = 0b10;
        let s = read_count(&page, |_| panic!("must not read a counter"));
        assert_eq!(s, dead(1_000));

        let page = FakePage::new(&[6, 6], 0);
        let s = read_count(&page, |_| panic!("must not read a counter"));
        assert_eq!(s, dead(1_000));
    }

    #[test]
    fn parses_cpufreq_khz() {
        assert_eq!(parse_khz("3600000\n"), Some(3.6e9));
        assert_eq!(parse_khz("0"), None);
        assert_eq!(parse_khz("fast"), None);
    }

    #[test]
    fn parses_sysctl_flag() {
        assert_eq!(parse_flag("1\n"), Some(true));
        assert_eq!(parse_flag("0"), Some(false));
        assert_eq!(parse_flag("2"), Some(true));
        assert_eq!(parse_flag(""), None);
    }

    #[test]
    fn cpu_list_ranges_and_singles() {
        let list = "0-3,8,10-11\n";
        for cpu in [0, 3, 8, 10, 11] {
            assert!(cpu_in_list(list, cpu), "cpu {cpu}");
        }
        for cpu in [4, 7, 9, 12] {
            assert!(!cpu_in_list(list, cpu), "cpu {cpu}");
        }
        assert!(!cpu_in_list("", 0));
        assert!(!cpu_in_list("x-y", 0));
    }
}
