use crate::tables::Tables;

include!(concat!(env!("OUT_DIR"), "/english_tables.rs"));

#[inline(always)]
pub fn english() -> &'static Tables {
    &ENGLISH
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::builder::build_english;

    #[test]
    fn static_tables_equal_runtime_build_field_by_field() {
        let built = build_english().expect("build");
        let s = english();
        assert!(s.lexicon == built.lexicon, "lexicon");
        assert!(s.role_of == built.role_of, "role_of");
        assert!(s.value_of == built.value_of, "value_of");
        assert!(s.shapes == built.shapes, "shapes");
        assert!(s.shape_len == built.shape_len, "shape_len");
        assert!(s.context == built.context, "context");
        assert!(s.candidates == built.candidates, "candidates");
        assert!(
            s.cardinal_default == built.cardinal_default,
            "cardinal_default"
        );
        assert!(s.initial_cells == built.initial_cells, "initial_cells");
        assert!(*s == *built);
    }

    #[test]
    fn static_tables_pass_validation() {
        assert_eq!(crate::builder::validate(english()), Ok(()));
    }
}
