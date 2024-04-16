pub(crate) mod crap;
pub(crate) mod skunk;
pub(crate) mod wcc;

use std::path::Path;

use rust_code_analysis::{get_function_spaces, guess_language, read_file, FuncSpace};
use serde::Serialize;
use tracing::debug;

use crate::{
    error::{Error, Result},
    Complexity, Thresholds,
};

const COVERAGE_THRESHOLD: f64 = 0.6;

#[derive(Debug, Serialize, Clone, Copy)]
#[serde(rename_all = "camelCase")]
pub(crate) struct MetricsThresholds {
    wcc: f64,
    crap_cyclomatic: f64,
    crap_cognitive: f64,
    skunk_cyclomatic: f64,
    skunk_cognitive: f64,
}

impl From<Thresholds> for MetricsThresholds {
    #[inline]
    fn from(value: Thresholds) -> Self {
        MetricsThresholds {
            wcc: value.wcc,
            crap_cyclomatic: crap::crap(COVERAGE_THRESHOLD, value.cyclomatic_complexity),
            crap_cognitive: crap::crap(COVERAGE_THRESHOLD, value.cognitive_complexity),
            skunk_cyclomatic: skunk::skunk(COVERAGE_THRESHOLD, value.cyclomatic_complexity),
            skunk_cognitive: skunk::skunk(COVERAGE_THRESHOLD, value.cognitive_complexity),
        }
    }
}

impl Default for MetricsThresholds {
    #[inline]
    fn default() -> Self {
        Self {
            wcc: 60.0,
            crap_cyclomatic: 16.4,
            crap_cognitive: 16.4,
            skunk_cyclomatic: 16.66,
            skunk_cognitive: 16.66,
        }
    }
}

impl MetricsThresholds {
    #[inline]
    pub(crate) fn is_complex(
        &self,
        wcc: f64,
        crap: f64,
        skunk: f64,
        complexity: Complexity,
    ) -> bool {
        let (crap_threshold, skunk_threshold) = match complexity {
            Complexity::Cyclomatic => (self.crap_cyclomatic, self.skunk_cyclomatic),
            Complexity::Cognitive => (self.crap_cognitive, self.skunk_cognitive),
        };

        wcc < self.wcc || crap > crap_threshold || skunk > skunk_threshold
    }
}

// Retrieve the root FuncSpace from a file.
#[inline]
pub(crate) fn get_root(path: &Path) -> Result<FuncSpace> {
    let source_code = read_file(path)?;
    let language = guess_language(&source_code, path)
        .0
        .ok_or(Error::Language)?;

    debug!("{:?} is written in {:?}", path, language);

    let root = get_function_spaces(&language, source_code, path, None).ok_or(Error::Metrics)?;

    Ok(root)
}

#[inline]
pub(crate) fn get_line_space(root: &FuncSpace, line: usize) -> &FuncSpace {
    let mut line_space = root;
    let mut stack = vec![root];
    while let Some(space) = stack.pop() {
        for s in &space.spaces {
            if (s.start_line + 1..s.end_line).contains(&line) {
                line_space = s;
                stack.push(s);
            }
        }
    }

    line_space
}

#[inline]
pub(crate) fn get_space_name(space: &FuncSpace) -> Option<String> {
    let name = format!(
        "{}({}, {})",
        space.name.as_ref()?,
        space.start_line,
        space.end_line
    );

    Some(name)
}

// Round f64 to first decimal.
#[inline]
pub(crate) fn round_sd(x: f64) -> f64 {
    (x * 10.0).round() / 10.0
}
