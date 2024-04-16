use super::round_sd;

// Computes the CRAP score given coverage and complexity of a FuncSpace.
// https://testing.googleblog.com/2011/02/this-code-is-crap.html#:~:text=CRAP%20is%20short%20for%20Change,partner%20in%20crime%20Bob%20Evans
#[inline]
pub(crate) fn crap(coverage: f64, complexity: f64) -> f64 {
    let crap = ((complexity.powf(2.)) * ((1.0 - coverage).powf(3.))) + complexity;

    round_sd(crap)
}
