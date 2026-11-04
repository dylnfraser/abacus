// Represents a scalar prefix used for unit conversion
#[derive(Debug)]
#[allow(dead_code)]
pub struct ScalarPrefix {
    pub scalar: f64,
    pub name: &'static str,
    pub alias: &'static str,
}

// Generates a static array of ScalarPrefix structs from a macro invocation
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
