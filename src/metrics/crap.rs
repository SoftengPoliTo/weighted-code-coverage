use rust_code_analysis::FuncSpace;

use crate::{concurrent::round_sd, error::*};

use super::get_covered_lines;

// Calculate the CRAP value for a given space
// (https://testing.googleblog.com/2011/02/this-code-is-crap.html#:~:text=CRAP%20is%20short%20for%20Change,partner%20in%20crime%20Bob%20Evans.)
// Return the value in case of success and an specif error in case of fails
pub(crate) fn crap_function(
    space: &FuncSpace,
    covs: &[Option<i32>],
    coverage: Option<f64>,
    comp: f64,
) -> Result<f64> {
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
    let crap = ((comp.powf(2.)) * ((1.0 - cov).powf(3.))) + comp;

    Ok(round_sd(crap))
}
