use core::cmp::Ordering;
use std::ffi::OsStr;
use std::fs;
use std::path::*;

use rust_code_analysis::{get_function_spaces, guess_language, read_file, FuncSpace, SpaceKind};
use tracing::debug;

use crate::concurrent::*;
use crate::error::*;
use crate::metrics::crap::*;
use crate::metrics::skunk::*;
use crate::metrics::wcc::*;
use crate::Complexity;

const COMPLEXITY_FACTOR: f64 = 25.0;

pub(crate) trait Visit {
    fn get_metrics_from_space(
        space: &FuncSpace,
        covs: &[Option<i32>],
        metric: Complexity,
        coverage: Option<f64>,
        thresholds: &[f64],
    ) -> Result<(Metrics, (f64, f64))>;
}
pub(crate) struct Tree;

impl Visit for Tree {
    fn get_metrics_from_space(
        space: &FuncSpace,
        covs: &[Option<i32>],
        metric: Complexity,
        coverage: Option<f64>,
        thresholds: &[f64],
    ) -> Result<(Metrics, (f64, f64))> {
        let (wcc_plain, sp_sum) = wcc_plain_function(space, covs, metric)?;
        let (wcc_quantized, sq_sum) = wcc_quantized_function(space, covs, metric)?;
        let crap = crap_function(space, covs, metric, coverage)?;
        let skunk = skunk_nosmells_function(space, covs, metric, coverage)?;
        let is_complex = check_complexity(wcc_plain, wcc_quantized, crap, skunk, thresholds);
        let coverage = if let Some(coverage) = coverage {
            coverage
        } else {
            let (covl, tl) = get_covered_lines(covs, space.start_line, space.end_line)?;
            if tl == 0.0 {
                0.0
            } else {
                (covl / tl) * 100.0
            }
        };
        let m = Metrics::new(
            wcc_plain,
            wcc_quantized,
            crap,
            skunk,
            is_complex,
            f64::round(coverage * 100.0) / 100.0,
        );
        Ok((m, (sp_sum, sq_sum)))
    }
}

#[inline]
pub(crate) fn get_prefix<A: AsRef<Path>>(files_path: A) -> Result<usize> {
    Ok(files_path
        .as_ref()
        .to_str()
        .ok_or(Error::PathConversion)?
        .to_string()
        .len())
}

// Chunks the vector of files in multiple chunks.
// Each chunk will contain a number of files equal, or very close, to `n_threads`.
pub(crate) fn chunk_vector(vec: Vec<String>, n_threads: usize) -> Vec<Vec<String>> {
    let chunks = vec.chunks((vec.len() / n_threads).max(1));
    chunks
        .map(|chunk| chunk.iter().map(|c| c.into()).collect::<Vec<String>>())
        .collect::<Vec<Vec<String>>>()
}

#[inline(always)]
#[allow(dead_code)]
pub(crate) fn compare_float(a: f64, b: f64) -> bool {
    a.total_cmp(&b) == Ordering::Equal
}

// Check all possible valid extensions
#[inline(always)]
fn check_ext(ext: &OsStr) -> bool {
    ext == "rs"
        || ext == "cpp"
        || ext == "c"
        || ext == "js"
        || ext == "java"
        || ext == "py"
        || ext == "tsx"
        || ext == "ts"
        || ext == "jsm"
}

// This function read all  the files in the project folder
// Returns all the source files, ignoring the other files or an error in case of problems
pub(crate) fn read_files<A: AsRef<Path>>(files_path: A) -> Result<Vec<String>> {
    let mut vec = vec![];
    let mut first = PathBuf::new();
    first.push(files_path);
    let mut stack = vec![first];
    while let Some(path) = stack.pop() {
        if path.is_dir() {
            let mut paths = fs::read_dir(&path)?;
            paths.try_for_each(|p| -> Result<()> {
                let pa = p?.path();
                stack.push(pa);
                Ok(())
            })?;
        } else {
            let ext = path.extension();

            if ext.is_some() && check_ext(ext.ok_or(Error::PathConversion)?) {
                vec.push(path.display().to_string().replace('\\', "/"));
            }
        }
    }
    Ok(vec)
}

// Get the code coverage in percentage between start and end
pub(crate) fn get_covered_lines(
    covs: &[Option<i32>],
    start: usize,
    end: usize,
) -> Result<(f64, f64)> {
    // Count the number of covered lines
    let (tot_lines, covered_lines) =
        covs.iter()
            .enumerate()
            .try_fold((0., 0.), |acc, (i, line)| -> Result<(f64, f64)> {
                let is_none = line.is_none();
                let sum;
                if !is_none && (start - 1..end).contains(&i) {
                    // let cov = line.as_u64().ok_or(Error::ConversionError())?;
                    // This unwrap can't panic because we have previously checked that `line` is not none
                    let cov = line.unwrap();
                    if cov > 0 {
                        sum = (acc.0 + 1., acc.1 + 1.);
                    } else {
                        sum = (acc.0 + 1., acc.1);
                    }
                } else {
                    sum = (acc.0, acc.1);
                }
                Ok(sum)
            })?;
    Ok((covered_lines, tot_lines))
}

// Get the root FuncSpace from a file
pub(crate) fn get_root<A: AsRef<Path>>(path: A) -> Result<FuncSpace> {
    let data = read_file(path.as_ref())?;
    let lang = guess_language(&data, path.as_ref())
        .0
        .ok_or(Error::Language)?;
    debug!("{:?} is written in {:?}", path.as_ref(), lang);
    let root = get_function_spaces(&lang, data, path.as_ref(), None).ok_or(Error::Metrics)?;
    Ok(root)
}

// Get all spaces stating from root.
// It does not contain the root
pub(crate) fn get_spaces(root: &FuncSpace) -> Result<Vec<(&FuncSpace, String)>> {
    let mut stack = vec![(root, String::new())];
    let mut result = Vec::new();
    while let Some((space, path)) = stack.pop() {
        for s in &space.spaces {
            let p = format!(
                "{}/{} ({},{})",
                path,
                s.name.as_ref().ok_or(Error::PathConversion)?,
                s.start_line,
                s.end_line
            );
            stack.push((s, p.to_string()));
            if s.kind == SpaceKind::Function {
                result.push((s, p));
            }
        }
    }
    Ok(result)
}

// Check complexity of a metric
// Return true if al least one metric exceed a threshold , false otherwise
#[inline(always)]
fn check_complexity(
    wcc_plain: f64,
    wcc_quantized: f64,
    crap: f64,
    skunk: f64,
    thresholds: &[f64],
) -> bool {
    wcc_plain > thresholds[0]
        || wcc_quantized > thresholds[1]
        || crap > thresholds[2]
        || skunk > thresholds[3]
}

// GET average, maximum and minimum given all the metrics
pub(crate) fn get_cumulative_values(metrics: &Vec<Metrics>) -> (Metrics, Metrics, Metrics) {
    let mut min = Metrics::min();
    let mut max = Metrics::default();
    let (wcc, wccq, crap, skunk, cov) = metrics.iter().fold((0.0, 0.0, 0.0, 0.0, 0.0), |acc, m| {
        max.wcc_plain = max.wcc_plain.max(m.wcc_plain);
        max.wcc_quantized = max.wcc_quantized.max(m.wcc_quantized);
        max.crap = max.crap.max(m.crap);
        max.skunk = max.skunk.max(m.skunk);
        min.wcc_plain = min.wcc_plain.min(m.wcc_plain);
        min.wcc_quantized = min.wcc_quantized.min(m.wcc_quantized);
        min.crap = min.crap.min(m.crap);
        min.skunk = min.skunk.min(m.skunk);
        (
            acc.0 + m.wcc_plain,
            acc.1 + m.wcc_quantized,
            acc.2 + m.crap,
            acc.3 + m.skunk,
            acc.4 + m.coverage,
        )
    });
    let l = metrics.len() as f64;
    let avg = Metrics::new(wcc / l, wccq / l, crap / l, skunk / l, false, cov);
    (avg, max, min)
}

// Calculate WCC PLAIN , WCC QUANTIZED, CRA and SKUNKSCORE for the entire project
// Using the sum values computed before
pub(crate) fn get_project_metrics(
    values: ConsumerOutputWcc,
    project_coverage: Option<f64>,
) -> Result<Metrics> {
    let project_coverage = if let Some(cov) = project_coverage {
        cov
    } else if values.total_lines != 0.0 {
        (values.covered_lines / values.total_lines) * 100.0
    } else {
        0.0
    };
    let mut m = Metrics::default();
    m = m.wcc_plain(values.wcc_plain_sum / values.ploc_sum);
    m = m.wcc_quantized(values.wcc_quantized_sum / values.ploc_sum);
    m = m.crap(
        ((values.comp_sum.powf(2.)) * ((1.0 - project_coverage / 100.).powf(3.))) + values.comp_sum,
    );
    m = m.skunk((values.comp_sum / COMPLEXITY_FACTOR) * (100. - (project_coverage)));
    m = m.coverage(project_coverage);
    Ok(m)
}
