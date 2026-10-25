use clm_itn::{Engine, FormatError, Formatted, Style};

fn plain_data<T: Send + Sync + Clone + Copy + std::fmt::Debug>() {}

#[test]
fn the_public_types_are_plain_data() {
    plain_data::<Engine<'static>>();
    plain_data::<Formatted>();
    plain_data::<FormatError>();
}

#[test]
fn english_and_default_build_the_same_engine() {
    let text = "at five thirty pm";
    assert_eq!(Engine::english().normalize(text), "at 5:30 pm");
    assert_eq!(Engine::default().normalize(text), "at 5:30 pm");
}

#[test]
fn normalize_returns_owned_text() {
    let engine = Engine::english();
    assert_eq!(engine.normalize("room four oh two."), "room 402.");
    assert_eq!(engine.normalize(""), "");
}

#[test]
fn normalize_into_appends_to_the_buffer() {
    let engine = Engine::english();
    let mut out = String::from("> ");
    engine.normalize_into("fifty percent off", &mut out);
    assert_eq!(out, "> 50% off");
}

#[test]
fn style_is_a_builder_and_shows_in_debug() {
    let ap = Engine::english();
    let digits = ap.with_style(Style::SingleDigits);
    assert_eq!(ap.style(), Style::Ap);
    assert_eq!(digits.style(), Style::SingleDigits);
    assert_eq!(ap.normalize("like eight years"), "like eight years");
    assert_eq!(digits.normalize("like eight years"), "like 8 years");
    assert!(format!("{digits:?}").contains("style: SingleDigits"));
}

#[test]
fn style_defaults_to_ap_and_is_plain_data() {
    fn traits<T: Send + Sync + Copy + Default + Eq + std::hash::Hash + std::fmt::Debug>() {}
    traits::<Style>();
    assert_eq!(Style::default(), Style::Ap);
    assert_eq!(Engine::default().style(), Style::default());
}

#[test]
fn format_needs_no_receipts() {
    let engine = Engine::english();
    let tokens: [&[u8]; 4] = [b"at", b"five", b"thirty", b"pm"];
    let mut out = [0u8; 64];
    let done = engine.format(&tokens, &mut out).expect("fits");
    assert_eq!(&out[..done.len], b"at 5:30 pm");
    assert_eq!(done.spans, 1);
}

#[test]
fn format_errors_are_std_errors_with_messages() {
    let engine = Engine::english();
    let mut out = [0u8; 2];
    let err = engine
        .format(&[b"five", b"pm"], &mut out)
        .expect_err("2 bytes cannot hold the output");
    assert_eq!(err, FormatError::Overflow);
    let boxed: Box<dyn std::error::Error + Send + Sync> = Box::new(err);
    assert!(boxed.to_string().contains("output buffer"));

    let nine: [&[u8]; 9] = [b"one"; 9];
    let err = engine.format(&nine, &mut [0u8; 64]).expect_err("9 > 8");
    assert_eq!(err, FormatError::TooManyTokens);
    assert!(err.to_string().contains("8"));
}
