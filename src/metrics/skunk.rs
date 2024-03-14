use super::round_sd;

pub(crate) const COMPLEXITY_FACTOR: f64 = 25.0;

// Computes the Skunk score given coverage and complexity of a FuncSpace.
// This implementation ignores code smells.
// https://www.fastruby.io/blog/code-quality/intruducing-skunk-stink-score-calculator.html
pub(crate) fn skunk(coverage: f64, complexity: f64) -> f64 {
    let skunk = if coverage == 100.0 {
        complexity / COMPLEXITY_FACTOR
    } else {
        (complexity / COMPLEXITY_FACTOR) * (100. - (100. * coverage))
    };

    round_sd(skunk)
}
