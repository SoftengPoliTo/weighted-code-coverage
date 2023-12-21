use rust_code_analysis::FuncSpace;

use crate::error::*;
use crate::Complexity;

use super::get_covered_lines;

const COMPLEXITY_FACTOR: f64 = 25.0;

// Calculate the Skunkscore value for a space
// https://www.fastruby.io/blog/code-quality/intruducing-skunk-stink-score-calculator.html
// In this implementation the code smells are ignored.
// Return the value in case of success and an specif error in case of fails
pub(crate) fn skunk_nosmells_function(
    space: &FuncSpace,
    covs: &[Option<i32>],
    metric: Complexity,
    coverage: Option<f64>,
) -> Result<f64> {
    let comp = match metric {
        Complexity::Cyclomatic => space.metrics.cyclomatic.cyclomatic_sum(),
        Complexity::Cognitive => space.metrics.cognitive.cognitive_sum(),
    };
    let cov = if let Some(coverage) = coverage {
        coverage / 100.0
    } else {
        let (covered_lines, tot_lines) = get_covered_lines(covs, space.start_line, space.end_line)?;
        if tot_lines != 0. {
            covered_lines / tot_lines
        } else {
            0.0
        }
    };
    Ok(if cov == 100. {
        comp / COMPLEXITY_FACTOR
    } else {
        (comp / COMPLEXITY_FACTOR) * (100. - (100. * cov))
    })
}
