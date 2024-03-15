use rust_code_analysis::FuncSpace;

use super::{round_sd, LinesMetrics};

const THRESHOLD: f64 = 15.;

pub(crate) struct WccFuncSpace {
    pub(crate) value: f64,
    pub(crate) percentage: f64,
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
pub(crate) fn wcc_file(wcc: f64, sloc: f64) -> WccFuncSpace {
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
