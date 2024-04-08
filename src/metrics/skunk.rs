use super::round_sd;

const COMPLEXITY_FACTOR: f64 = 0.6;

// Computes the Skunk score given coverage and complexity of a FuncSpace.
// This implementation ignores code smells.
// https://www.fastruby.io/blog/code-quality/intruducing-skunk-stink-score-calculator.html
#[inline]
pub(crate) fn skunk(coverage: f64, complexity: f64) -> f64 {
    let skunk = ((complexity / COMPLEXITY_FACTOR) * (1.0 - coverage)) + complexity;

    round_sd(skunk)
}
