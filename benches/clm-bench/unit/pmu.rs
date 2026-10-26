use crate::pmu::*;
use core::cell::{Cell, RefCell};
use core::mem::{offset_of, size_of};

const FLAG_DISABLED: u64 = 1 << 0;

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
