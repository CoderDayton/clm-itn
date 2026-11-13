use crate::tables::Tables;

include!(concat!(env!("OUT_DIR"), "/portuguese_tables.rs"));

#[inline(always)]
pub fn portuguese() -> &'static Tables {
    &PORTUGUESE
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::builder::build_portuguese;

    #[test]
    fn static_tables_equal_runtime_build() {
        let built = build_portuguese().expect("build");
        assert!(*portuguese() == *built);
    }

    #[test]
    fn static_tables_pass_validation() {
        assert_eq!(crate::builder::validate(portuguese()), Ok(()));
    }
}
