use crate::tables::Tables;

include!(concat!(env!("OUT_DIR"), "/german_tables.rs"));

#[inline(always)]
pub fn german() -> &'static Tables {
    &GERMAN
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::builder::build_german;

    #[test]
    fn static_tables_equal_runtime_build() {
        let built = build_german().expect("build");
        assert!(*german() == *built);
    }

    #[test]
    fn static_tables_pass_validation() {
        assert_eq!(crate::builder::validate(german()), Ok(()));
    }
}
