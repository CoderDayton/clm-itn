use crate::tables::Tables;

include!(concat!(env!("OUT_DIR"), "/spanish_tables.rs"));

#[inline(always)]
pub fn spanish() -> &'static Tables {
    &SPANISH
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::builder::build_spanish;

    #[test]
    fn static_tables_equal_runtime_build() {
        let built = build_spanish().expect("build");
        assert!(*spanish() == *built);
    }

    #[test]
    fn static_tables_pass_validation() {
        assert_eq!(crate::builder::validate(spanish()), Ok(()));
    }
}
