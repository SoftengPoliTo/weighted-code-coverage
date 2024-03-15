pub mod crap;
pub mod skunk;
pub mod wcc;

use std::path::Path;

use rust_code_analysis::{get_function_spaces, guess_language, read_file, FuncSpace};
use tracing::debug;

use crate::error::{Error, Result};

#[derive(Debug, Default)]
pub(crate) struct LinesMetrics {
    pub(crate) total_lines: f64,
    pub(crate) covered_lines: f64,
}

impl LinesMetrics {
    pub(crate) fn new(space: &FuncSpace, lines_coverage: &[Option<i32>]) -> Self {
        let mut total_lines = 0.0;
        let mut covered_lines = 0.0;
        lines_coverage
            .iter()
            .enumerate()
            .filter(|(line_number, _)| (space.start_line - 1..space.end_line).contains(line_number))
            .for_each(|(_, coverage)| {
                total_lines += 1.0;
                if coverage.is_some() {
                    covered_lines += 1.0;
                }
            });

        Self {
            total_lines,
            covered_lines,
        }
    }

    pub(crate) fn update(&mut self, other: LinesMetrics) {
        self.total_lines += other.total_lines;
        self.covered_lines += other.covered_lines;
    }

    pub(crate) fn get_covered_lines(space: &FuncSpace, lines_coverage: &[Option<i32>]) -> f64 {
        Self::new(space, lines_coverage).covered_lines
    }
}

// Get the root FuncSpace from a file.
pub(crate) fn get_root(path: &Path) -> Result<FuncSpace> {
    let source_code = read_file(path)?;
    let language = guess_language(&source_code, path)
        .0
        .ok_or(Error::Language)?;

    debug!("{:?} is written in {:?}", path, language);

    let root = get_function_spaces(&language, source_code, path, None).ok_or(Error::Metrics)?;

    Ok(root)
}

// Round f64 to second decimal.
#[inline]
pub(crate) fn round_sd(x: f64) -> f64 {
    (x * 100.0).round() / 100.0
}
