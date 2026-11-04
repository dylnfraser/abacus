// A unit prefix (`kilo`/`k`) and its scale factor.
#[derive(Debug)]
#[allow(dead_code)]
pub struct ScalarPrefix {
    pub scalar: f64,
    pub name: &'static str,
    pub alias: &'static str,
}

// Builds a static `ScalarPrefix` slice from `name, alias, scalar;` triples.
macro_rules! gen_prefixes {
    ( $( $name:expr, $alias:expr, $scalar:expr );* $(;)? ) => {
        &[
            $(
                ScalarPrefix {
                    name: $name,
                    alias: $alias,
                    scalar: $scalar,
                }
            ),*
        ]
    };
}

pub(crate) use gen_prefixes;
