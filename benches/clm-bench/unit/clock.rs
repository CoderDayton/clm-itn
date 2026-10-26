use crate::clock::*;

#[test]
fn ticks_to_ns_scales_by_rate() {
    assert_eq!(ticks_to_ns(0, 0.25), 0.0);
    assert_eq!(ticks_to_ns(400, 0.25), 100.0);
    assert_eq!(ticks_to_ns(3, 41.0 + 2.0 / 3.0), 125.0);
}

#[test]
fn gate_passes_at_and_below_ten_ns_with_invariant_tsc() {
    assert!(gate_decision(0.25, true));
    assert!(gate_decision(10.0, true));
}

#[test]
fn gate_fails_above_ten_ns() {
    assert!(!gate_decision(10.01, true));
    assert!(!gate_decision(41.67, true));
}

#[test]
fn gate_fails_without_invariant_tsc() {
    assert!(!gate_decision(0.25, false));
}

#[test]
fn leaf15_rate_is_crystal_times_ratio() {
    assert_eq!(rate_from_leaf15(2, 188, 38_400_000), Some(3_609_600_000.0));
}

#[test]
fn leaf15_unreported_rate_is_none() {
    assert_eq!(rate_from_leaf15(2, 188, 0), None);
    assert_eq!(rate_from_leaf15(2, 0, 38_400_000), None);
    assert_eq!(rate_from_leaf15(0, 188, 38_400_000), None);
}

#[test]
fn fine_generic_counter_skips_pmu() {
    let s = select(1.0, || panic!("PMU must not be tried"));
    assert_eq!(s.source, Source::Generic);
    assert_eq!(s.ns_per_tick, 1.0);
    assert!(gate_decision(s.ns_per_tick, s.source != Source::Coarse));
}

#[test]
fn coarse_generic_counter_uses_available_pmu() {
    let s = select(41.67, || Ok((0.31, "3.2 GHz".into())));
    assert_eq!(s.source, Source::Pmu);
    assert_eq!(s.ns_per_tick, 0.31);
    assert!(s.note.unwrap().contains("3.2 GHz"));
    assert!(gate_decision(s.ns_per_tick, s.source != Source::Coarse));
}

#[test]
fn coarse_generic_counter_without_pmu_stays_coarse_with_reason() {
    let s = select(41.67, || Err("perf_user_access=0".into()));
    assert_eq!(s.source, Source::Coarse);
    assert_eq!(s.ns_per_tick, 41.67);
    assert!(s.note.unwrap().contains("perf_user_access=0"));
    assert!(!gate_decision(s.ns_per_tick, s.source != Source::Coarse));
}

#[test]
fn median_of_odd_and_even_counts() {
    assert_eq!(median(&mut [5, 1, 3]), 3);
    assert_eq!(median(&mut [4, 1, 3, 2]), 3);
    assert_eq!(median(&mut [7]), 7);
}
