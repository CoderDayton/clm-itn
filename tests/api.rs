use clm_itn::{Normalizer, Setting};

#[test]
fn the_normalizer_is_plain_data() {
    fn plain_data<T: Send + Sync + Clone + Copy + std::fmt::Debug>() {}
    plain_data::<Normalizer<'static>>();
}

#[test]
fn english_and_default_build_the_same_engine() {
    let text = "at five thirty pm";
    assert_eq!(Normalizer::english().normalize(text), "at 5:30 pm");
    assert_eq!(Normalizer::default().normalize(text), "at 5:30 pm");
}

#[test]
fn normalize_returns_owned_text() {
    let engine = Normalizer::english();
    assert_eq!(engine.normalize("room four oh two."), "room 402.");
    assert_eq!(engine.normalize(""), "");
}

#[test]
fn normalize_into_appends_to_the_buffer() {
    let engine = Normalizer::english();
    let mut out = String::from("> ");
    engine.normalize_into("fifty percent off", &mut out);
    assert_eq!(out, "> 50% off");
}

#[test]
fn every_setting_is_on_by_default_and_switches_off_alone() {
    let all = Normalizer::english();
    for &s in Setting::ALL {
        assert!(all.is_on(s), "{s:?}");
        let off = all.without(s);
        assert!(!off.is_on(s), "{s:?}");
        assert!(Setting::ALL.iter().all(|&o| o == s || off.is_on(o)));
        assert!(off.with(s).is_on(s), "{s:?}");
    }
    assert!(Normalizer::default().is_on(Setting::SingleDigits));
    let off = all.without(Setting::Percent);
    assert!(format!("{off:?}").contains("Percent"), "{off:?}");
}

#[test]
fn setting_names_round_trip_and_settings_are_plain_data() {
    fn traits<T: Send + Sync + Copy + Eq + std::hash::Hash + std::fmt::Debug>() {}
    traits::<Setting>();
    for &s in Setting::ALL {
        assert_eq!(Setting::from_name(s.name()), Some(s));
    }
    assert_eq!(Setting::SingleDigits.name(), "single-digits");
    assert_eq!(Setting::from_name("nope"), None);
}
