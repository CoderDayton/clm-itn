use clm_itn::{Normalizer, Setting};

const CASES: &[(&str, &str)] = &[
    ("tengo tres gatos", "tengo tres gatos"),
    ("uno punto treinta y tres", "1,33"),
    ("nueve punto cinco millones de pesos", "$9,5 millones"),
    ("dos millardos cincuenta y dos", "2000000052"),
    ("setenta y cinco dólares con sesenta y tres", "$75,63"),
    ("diez para las doce", "diez para las doce"),
    ("un cuarto para las dos", "un cuarto para las dos"),
    ("las veintitrés y media", "las veintitrés y media"),
    ("las dos menos veinte", "las dos menos veinte"),
    ("a las tres y diez", "a las 3:10"),
    ("cien dólares y noventa y nueve centavos", "$100,99"),
    (
        "pagué ciento noventa y nueve dólares y noventa y nueve centavos ayer",
        "pagué $199,99 ayer",
    ),
    ("las dos a eme", "las 2 am"),
    ("la una y diez pe eme", "la 1:10 pm"),
    ("una casa a eme", "una casa a eme"),
    ("doce dólares estadounidenses y cinco centavos", "US$12,05"),
    ("diez pesos mexicanos", "Mex$10"),
    ("los dólares estadounidenses", "los dólares estadounidenses"),
    ("doce yenes", "¥12"),
    ("tres coma catorce quince", "3,1415"),
    ("tres coma cero ciento cuarenta y uno", "3,0141"),
    (
        "tres coma ciento cuarenta y uno cincuenta y nueve",
        "3,14159",
    ),
    ("son veinticinco personas", "son 25 personas"),
    ("treinta y cinco", "35"),
    ("doscientos treinta y cinco", "235"),
    ("ciento uno", "101"),
    ("cien mil", "100000"),
    ("mil novecientos noventa y dos", "1992"),
    ("dos millones tres", "2000003"),
    ("un millón", "1 millón"),
    ("tres millones de personas", "3 millones de personas"),
    (
        "ciento cuarenta y siete mil cuatrocientos cincuenta y uno",
        "147451",
    ),
    ("menos veintitrés", "-23"),
    ("tres coma catorce", "3,14"),
    ("cero coma seis millones", "0,6 millones"),
    ("cuesta doce dólares y cinco centavos", "cuesta $12,05"),
    ("veinte euros con cincuenta", "€20,50"),
    (
        "pagué setenta y cinco dólares y sesenta y tres centavos ayer",
        "pagué $75,63 ayer",
    ),
    ("cincuenta por ciento", "50 %"),
    ("doscientos metros", "200 metros"),
    ("llegamos a las tres y diez", "llegamos a las 3:10"),
    ("a la una y cuarenta y cinco", "a la 1:45"),
    ("las dieciséis cincuenta", "las 16:50"),
    ("la una y la otra", "la una y la otra"),
    ("nació el cinco de mayo", "nació el 5 de mayo"),
    (
        "el treinta y uno de diciembre de mil novecientos noventa y dos",
        "el 31 de diciembre de 1992",
    ),
    ("el año dos mil veinticuatro", "el año 2024"),
    ("en el año mil novecientos noventa", "en el año 1990"),
    ("la semana treinta y dos", "la semana 32"),
    ("dos de los niños", "dos de los niños"),
    ("café con leche", "café con leche"),
    ("tres coma cero cero cinco", "3,005"),
    ("menos sesenta coma dos cuatro cero cero", "-60,2400"),
    ("entre dieciséis y dieciocho", "entre 16 y 18"),
    ("dieciséis mil y dieciocho mil", "16000 y 18000"),
    ("treinta y cuarenta", "30 y 40"),
    ("dos mil millones", "2 mil millones"),
    ("doscientos mil millones de dólares", "$200 mil millones"),
    ("mil ochocientos veinticuatro millones", "1824 millones"),
    ("mil millones uno", "1000000001"),
    ("mil millones de pesos", "mil millones de pesos"),
    (
        "mil quinientos noventa y tres millones setenta y dos mil novecientos sesenta y uno",
        "1593072961",
    ),
    ("veintinueve dólares cincuenta centavos", "$29,50"),
    ("dos millones de dólares", "$2 millones"),
    ("dos millones de personas", "2 millones de personas"),
];

#[test]
fn spoken_spanish_becomes_written() {
    let n = Normalizer::spanish();
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
fn settings_switch_spanish_rewrites_off() {
    let n = Normalizer::spanish().without(Setting::Percent);
    assert_eq!(n.normalize("cincuenta por ciento"), "50 por ciento");
    let n = Normalizer::spanish().without(Setting::Currency);
    assert_eq!(n.normalize("doce euros"), "12 euros");
}

#[test]
fn english_reads_no_spanish() {
    let n = Normalizer::english();
    assert_eq!(n.normalize("treinta y cinco"), "treinta y cinco");
}
