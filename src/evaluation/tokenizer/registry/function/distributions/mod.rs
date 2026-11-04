#[cfg(any(feature = "distributions", feature = "stats"))]
pub mod binomial;
#[cfg(any(feature = "distributions", feature = "stats"))]
pub mod chi_square;
#[cfg(any(feature = "distributions", feature = "stats"))]
pub mod exponential;
#[cfg(any(feature = "distributions", feature = "stats"))]
pub mod f_dist;
#[cfg(any(feature = "distributions", feature = "stats"))]
pub mod geometric;
#[cfg(any(feature = "distributions", feature = "stats"))]
pub mod hypergeometric;
#[cfg(any(feature = "distributions", feature = "stats"))]
pub mod normal;
#[cfg(any(feature = "distributions", feature = "stats"))]
pub mod poisson;
pub mod special;
#[cfg(any(feature = "distributions", feature = "stats"))]
pub mod student_t;
#[cfg(any(feature = "distributions", feature = "stats"))]
pub mod uniform;

#[cfg(any(feature = "distributions", feature = "stats"))]
use crate::evaluation::tokenizer::registry::function::operators::FunctionOp;

// Statistics uses the distributions for p-values and critical values.
#[cfg(any(feature = "distributions", feature = "stats"))]
#[must_use]
pub fn register_distributions() -> Vec<FunctionOp> {
    let mut functions = Vec::new();
    functions.append(&mut binomial::register_binomial());
    functions.append(&mut geometric::register_geometric());
    functions.append(&mut poisson::register_poisson());
    functions.append(&mut normal::register_normal());
    functions.append(&mut student_t::register_student_t());
    functions.append(&mut chi_square::register_chi_square());
    functions.append(&mut f_dist::register_f_dist());
    functions.append(&mut exponential::register_exponential());
    functions.append(&mut hypergeometric::register_hypergeometric());
    functions.append(&mut uniform::register_uniform());
    functions
}
