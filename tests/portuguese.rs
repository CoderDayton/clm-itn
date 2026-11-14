use clm_itn::{Normalizer, Setting};

const CASES: &[(&str, &str)] = &[
    ("tenho três gatos", "tenho três gatos"),
    ("são vinte e cinco pessoas", "são 25 pessoas"),
    ("vinte e três", "23"),
    ("trinta e três", "33"),
    ("menos vinte e três", "-23"),
    ("cem", "100"),
    ("cento e um", "101"),
    ("cento e uma", "101"),
    ("cento e vinte e três", "123"),
    ("duzentos", "200"),
    ("duzentos e cinquenta e um", "251"),
    ("mil e um", "1001"),
    ("mil e cem", "1100"),
    ("mil cento e vinte e seis", "1126"),
    ("dois mil e vinte e dois", "2022"),
    ("mil novecentos e noventa e dois", "1992"),
    ("dois milhões e três", "2000003"),
    ("dois bilhões e cinquenta e dois", "2000000052"),
    (
        "cento e quarenta e sete mil quatrocentos e cinquenta e um",
        "147451",
    ),
    (
        "dezoito milhões quatrocentos e cinquenta mil novecentos e noventa",
        "18450990",
    ),
    (
        "quarenta e seis mil seiscentos e sessenta e quatro",
        "46664",
    ),
    ("um trilhão e um", "1000000000001"),
    ("dois trilhões e quinhentos", "2000000000500"),
    ("um quatrilhão e um", "1000000000000001"),
    ("um milhão e um", "1000001"),
    ("um milhão", "1 milhão"),
    ("dois bilhões", "2 bilhões"),
    ("três milhões de pessoas", "3 milhões de pessoas"),
    ("muitos milhões", "muitos milhões"),
    ("um vírgula dois seis", "1,26"),
    ("menos um vírgula dois seis", "-1,26"),
    ("zero vírgula vinte e seis", "0,26"),
    ("três vírgula catorze", "3,14"),
    ("um ponto trinta e três", "1,33"),
    ("zero vírgula seis milhões", "0,6 milhões"),
    (
        "três vírgula cento e quarenta e um cinquenta e nove",
        "3,14159",
    ),
    ("três vírgula quatorze quinze noventa e dois", "3,141592"),
    ("três vírgula zero cento e quarenta e um", "3,0141"),
    ("doze dólares e cinco centavos", "$ 12,05"),
    ("setenta e cinco dólares com sessenta e três", "$ 75,63"),
    (
        "setenta e cinco dólares e sessenta e três centavos",
        "$ 75,63",
    ),
    ("um real", "R$ 1"),
    ("cem reais", "R$ 100"),
    ("cento e noventa e nove reais", "R$ 199"),
    ("vinte e cinco centavos", "25 centavos"),
    ("doze euros e cinco centavos", "€ 12,05"),
    ("doze dólares americanos e cinco centavos", "US$ 12,05"),
    ("duas libras esterlinas", "£ 2"),
    ("duas libras", "2 libras"),
    (
        "pagamos cento e quinze reais por uma bala",
        "pagamos R$ 115 por uma bala",
    ),
    ("quinze mil reais", "R$ 15000"),
    ("cem reais e noventa e nove centavos", "R$ 100,99"),
    (
        "cento e noventa e nove reais e noventa e nove centavos",
        "R$ 199,99",
    ),
    ("dez para vinte pessoas", "10 para 20 pessoas"),
    ("cinquenta por cento", "50 %"),
    ("um por cento", "1 %"),
    ("duzentos metros", "200 metros"),
    ("dois quilos", "2 quilos"),
    ("às duas e dez", "às 2:10"),
    ("chegaram às três e quinze", "chegaram às 3:15"),
    ("à uma e trinta", "à 1:30"),
    ("às dezesseis e cinquenta", "às 16:50"),
    ("às duas e meia", "às duas e meia"),
    ("às quinze pras duas", "às quinze pras duas"),
    ("dez para as duas", "dez para as duas"),
    ("vinte e cinco para as três", "vinte e cinco para as três"),
    ("quinze para o meio dia", "quinze para o meio dia"),
    ("à meia noite e quinze", "à meia noite e quinze"),
    ("ao meio dia e vinte e cinco", "ao meio dia e vinte e cinco"),
    ("meia noite e meia", "meia noite e meia"),
    ("nasceu em cinco de maio", "nasceu em 5 de maio"),
    (
        "trinta e um de dezembro de mil novecentos e oitenta e oito",
        "31 de dezembro de 1988",
    ),
    ("o ano dois mil e vinte e quatro", "o ano 2024"),
    ("no ano mil novecentos e noventa", "no ano 1990"),
    ("a semana trinta e dois", "a semana 32"),
    ("dois dos meninos", "dois dos meninos"),
    ("um e outro", "um e outro"),
    ("café com leite", "café com leite"),
    ("três vírgula zero zero cinco", "3,005"),
    ("menos sessenta vírgula dois quatro zero zero", "-60,2400"),
    ("zero vírgula vinte seis", "0,26"),
    ("vinte três", "23"),
    ("cento e vinte três", "123"),
    ("entre dezesseis e dezoito", "entre 16 e 18"),
    ("vinte e trinta", "20 e 30"),
    ("dois mil e três mil", "2000 e 3000"),
    ("às onze e trinta", "às 11:30"),
    ("um bilhão e mil cento e um", "1000001101"),
    ("um milhão e mil", "1001000"),
    ("às quinze pra meia noite", "às quinze pra meia noite"),
    ("vinte reais cinquenta centavos", "R$ 20,50"),
    ("dois milhões de reais", "R$ 2 milhões"),
    ("um milhão de pessoas", "1 milhão de pessoas"),
];

#[test]
fn spoken_portuguese_becomes_written() {
    let n = Normalizer::portuguese();
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
fn settings_switch_portuguese_rewrites_off() {
    let n = Normalizer::portuguese().without(Setting::Percent);
    assert_eq!(n.normalize("cinquenta por cento"), "50 por cento");
    let n = Normalizer::portuguese().without(Setting::Currency);
    assert_eq!(n.normalize("doze reais"), "12 reais");
}

#[test]
fn other_languages_read_no_portuguese() {
    assert_eq!(
        Normalizer::english().normalize("vinte e três"),
        "vinte e três"
    );
    assert_eq!(
        Normalizer::spanish().normalize("duzentos e cinquenta"),
        "duzentos e cinquenta"
    );
}
