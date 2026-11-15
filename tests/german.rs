#![cfg(feature = "german")]

use clm_itn::{Normalizer, Setting};

const CASES: &[(&str, &str)] = &[
    ("ich habe drei Katzen", "ich habe drei Katzen"),
    ("es sind fünfundzwanzig Leute", "es sind 25 Leute"),
    ("dreiundzwanzig", "23"),
    ("siebenundzwanzig", "27"),
    ("Siebenunddreißig", "37"),
    ("sechs und fünfzig", "56"),
    ("minus dreiundzwanzig", "-23"),
    ("hundert", "100"),
    ("einhundert", "100"),
    ("ein hundert und zwei", "102"),
    ("hunderteins", "101"),
    ("Zweihundertfünfundvierzig", "245"),
    ("zwei hundert fünf und vierzig", "245"),
    ("zweihundertsiebenundneunzig", "297"),
    ("tausendundeins Nacht", "1001 Nacht"),
    ("eintausend", "1000"),
    ("zweitausendvierundzwanzig", "2024"),
    ("zwei tausend vier und zwanzig", "2024"),
    ("zweihundertfünfundvierzigtausenddreihundertzwölf", "245312"),
    ("das Jahr neunzehnhundertneunundneunzig", "das Jahr 1999"),
    ("neunzehn hundert neunzig", "1990"),
    ("eine Million", "1 Million"),
    ("zwei Millionen", "2 Millionen"),
    ("zwei Millionen dreihunderttausend", "2300000"),
    ("drei Milliarden", "3 Milliarden"),
    ("drei Komma eins vier", "3,14"),
    ("null Komma fünf", "0,5"),
    ("drei Komma null null", "3,00"),
    ("minus sechzig Komma zwei vier null null", "-60,2400"),
    ("fünf Euro", "5 €"),
    ("zwölf Euro fünfzig", "12,50 €"),
    ("zwölf Euro und fünfzig Cent", "12,50 €"),
    ("zweihundertsiebenundneunzig Euro", "297 €"),
    ("zwei Dollar zwanzig", "2,20 $"),
    ("fünfzig Cent", "50 Cent"),
    ("zehn Prozent", "10 %"),
    ("minus zwanzig Grad", "-20 Grad"),
    ("dreihundert Meter", "300 Meter"),
    ("einhundertzwei Kilometer", "102 Kilometer"),
    ("zwei Kilowattstunden", "2 Kilowattstunden"),
    ("um vierzehn Uhr dreißig", "um 14 Uhr 30"),
    ("halb drei", "halb drei"),
    ("zehn vor drei", "zehn vor drei"),
    ("Viertel nach vier", "Viertel nach vier"),
    ("fünf nach halb vier", "fünf nach halb vier"),
    ("drei vor zwölf", "drei vor zwölf"),
    ("Nummer vier fünf sechs", "Nummer 456"),
    ("ein Haus", "ein Haus"),
    ("Einkaufswagen", "Einkaufswagen"),
    ("Hunderte von Menschen", "Hunderte von Menschen"),
    ("Tausende Menschen", "Tausende Menschen"),
    ("Brot und Butter", "Brot und Butter"),
    ("zwischen zwanzig und dreißig", "zwischen 20 und 30"),
    ("zweihundertundfünfundvierzig", "245"),
    ("zwounddreissig", "32"),
    ("tausendundeins", "1001"),
    ("acht uhr", "8 uhr"),
    ("acht uhr sieben", "8 uhr 7"),
    ("zwei uhren", "zwei uhren"),
    ("Eine Stunde später.", "Eine Stunde später."),
    ("eine minute", "eine minute"),
    ("ein tag", "ein tag"),
    ("eine stunde und eine halbe", "eine stunde und eine halbe"),
    ("eine stunde und zwanzig minuten", "1 stunde und 20 minuten"),
    ("einundzwanzig stunden", "21 stunden"),
    ("zwei stunden", "2 stunden"),
];

#[test]
fn spoken_german_becomes_written() {
    let n = Normalizer::german();
    let wrong: Vec<String> = CASES
        .iter()
        .filter_map(|&(spoken, written)| {
            let got = n.normalize(spoken);
            (got != written).then(|| format!("{spoken:?}: want {written:?}, got {got:?}"))
        })
        .collect();
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
}

#[test]
fn settings_switch_german_rewrites_off() {
    let n = Normalizer::german().without(Setting::Percent);
    assert_eq!(n.normalize("fünfzig Prozent"), "50 Prozent");
    let n = Normalizer::german().without(Setting::Currency);
    assert_eq!(n.normalize("zwölf Euro"), "12 Euro");
}

#[test]
fn other_languages_read_no_german() {
    assert_eq!(
        Normalizer::english().normalize("zweihundertfünfundvierzig"),
        "zweihundertfünfundvierzig"
    );
    assert_eq!(
        Normalizer::spanish().normalize("dreiundzwanzig"),
        "dreiundzwanzig"
    );
}
