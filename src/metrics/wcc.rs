use rust_code_analysis::FuncSpace;
use tracing::debug;

use crate::{error::*, Complexity};

use super::{round_sd, LinesMetrics};

pub(crate) struct WccFuncSpace {
    pub(crate) value: f64,
    pub(crate) percentage: f64,
}

const THRESHOLD: f64 = 15.;
// This function find the minimum space for a line i in the file
// It returns the space
fn get_min_space(root: &FuncSpace, i: usize) -> FuncSpace {
    let mut min_space: FuncSpace = root.clone();
    let mut stack: Vec<FuncSpace> = vec![root.clone()];
    while let Some(space) = stack.pop() {
        for s in space.spaces.into_iter() {
            if i >= s.start_line && i <= s.end_line {
                min_space = s.clone();
                stack.push(s);
            }
        }
    }
    min_space
}

// Calculate the WCC plain value for a space
// Return the value in case of success and an specif error in case of fails
pub(crate) fn wcc_plain_function(
    space: &FuncSpace,
    lines_coverage: &[Option<i32>],
    complexity: f64,
) -> Result<(f64, f64, f64)> {
    let sloc = space.metrics.loc.sloc();
    let sum = lines_coverage
        .iter()
        .enumerate()
        .try_fold(0., |acc, (i, line)| -> Result<f64> {
            let mut sum = acc;
            let start = space.start_line;
            let end = space.end_line + 1;
            if let Some(cov) = *line {
                if cov > 0 && (start..end).contains(&i) {
                    // If the line is not null and is covered (cov>0) the add the complexity to the sum
                    sum = acc + complexity;
                }
            }

            Ok(sum)
        })?;
    let wcc_plain = (sum / (complexity * sloc)) * 100.0;

    Ok((round_sd(wcc_plain), sum, complexity * sloc))
}

// Computes both the raw and percentage weighted code coverage values for a given FuncSpace.
pub(crate) fn wcc_func_space(
    space: &FuncSpace,
    lines_coverage: &[Option<i32>],
    complexity: f64,
) -> WccFuncSpace {
    if complexity >= THRESHOLD || space.metrics.loc.sloc() == 0.0 {
        return WccFuncSpace {
            value: 0.0,
            percentage: 0.0,
        };
    }
    let covered_lines = LinesMetrics::get_covered_lines(space, lines_coverage);

    WccFuncSpace {
        value: covered_lines,
        percentage: round_sd((covered_lines / space.metrics.loc.sloc()) * 100.0),
    }
}

// Computes both the raw and percentage weighted code coverage values for a given file.
pub(crate) fn wcc_file(
    wcc: f64,
    sloc: f64
) -> WccFuncSpace {
    if sloc == 0.0 {
        return WccFuncSpace {
            value: 0.0,
            percentage: 0.0,
        };
    }

    WccFuncSpace {
        value: wcc,
        percentage: round_sd((wcc / sloc) * 100.0),
    }
}

// Calculate the WCC quantized value for a space
// Return the value in case of success and an specif error in case of fails
// If the complexity of the block/file is 0 the value if wcc quantized is the coverage of the file
pub(crate) fn wcc_quantized_function(
    space: &FuncSpace,
    lines_coverage: &[Option<i32>],
    metric: Complexity,
) -> Result<(f64, f64, f64)> {
    let sloc = space.metrics.loc.sloc();
    let sum =
    //For each line find the minimum space and get complexity value then sum 1 if comp>threshold  else sum 1
        lines_coverage.iter()
            .enumerate()
            .try_fold(0., |acc, (i, line)| -> Result<f64> {
                let mut sum = acc;
                let start = space.start_line;
                let end = space.end_line + 1;
                if let Some(cov) = *line {
                    if cov > 0 &&  (start..end).contains(&i) {
                        // If the line is covered get the space of the line and then check if the complexity is below the threshold
                        let min_space: FuncSpace = get_min_space(space, i);
                        let comp = match metric {
                            Complexity::Cyclomatic => min_space.metrics.cyclomatic.cyclomatic(),
                            Complexity::Cognitive => min_space.metrics.cognitive.cognitive(),
                        };
                        if comp > THRESHOLD {
                            sum = acc + 2.;
                        } else {
                            sum = acc + 1.;
                        }
                    }
                }

                Ok(sum)
            })?;
    let wcc_quantized = (sum / (2.0 * sloc)) * 100.0;

    Ok((round_sd(wcc_quantized), sum, 2.0 * sloc))
}
