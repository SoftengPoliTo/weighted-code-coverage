use core::cmp::Ordering;
use std::collections::HashMap;
use std::ffi::OsStr;
use std::fmt;
use std::fs;
use std::path::*;
use std::str::FromStr;

use rust_code_analysis::{get_function_spaces, guess_language, read_file, FuncSpace, SpaceKind};
use serde_json::Map;
use serde_json::Value;
use tracing::debug;

use crate::error::*;
use crate::files::*;
use crate::metrics::crap::*;
use crate::metrics::skunk::*;
use crate::metrics::wcc::*;

const COMPLEXITY_FACTOR: f64 = 25.0;

/// Complexity Metrics
#[derive(Copy, Debug, Clone, PartialEq, Eq, Hash)]
pub enum Complexity {
    /// Cyclomatic metric.
    Cyclomatic,
    /// Cognitive metric.
    Cognitive,
}

impl fmt::Display for Complexity {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self)
    }
}

impl FromStr for Complexity {
    type Err = String;

    fn from_str(complexity: &str) -> std::result::Result<Self, Self::Err> {
        match complexity {
            "cyclomatic" => Ok(Complexity::Cyclomatic),
            "cognitive" => Ok(Complexity::Cognitive),
            _ => Err(format!("{complexity:?} is not a supported complexity.")),
        }
    }
}

impl Complexity {
    /// All complexity formats.
    pub const fn all() -> &'static [&'static str] {
        &["cyclomatic", "cognitive"]
    }

    /// Default complexity format.
    pub const fn default() -> &'static str {
        "cyclomatic"
    }
}

/// JSONs format available
#[derive(Copy, Debug, Clone, PartialEq, Eq, Hash)]
pub enum JsonFormat {
    /// Cyclomatic metric.
    Covdir,
    /// Cognitive metric.
    Coveralls,
}

impl FromStr for JsonFormat {
    type Err = String;

    fn from_str(json_type: &str) -> std::result::Result<Self, Self::Err> {
        match json_type {
            "covdir" => Ok(JsonFormat::Covdir),
            "coveralls" => Ok(JsonFormat::Coveralls),
            _ => Err(format!(
                "{json_type:?} is not a supported code coverage output format."
            )),
        }
    }
}

impl JsonFormat {
    /// All output formats.
    pub const fn all() -> &'static [&'static str] {
        &["covdir", "coveralls"]
    }

    /// Default output format.
    pub const fn default() -> &'static str {
        "coveralls"
    }
}

/// Mode
#[derive(Copy, Debug, Clone, PartialEq, Eq, Hash)]
pub enum Mode {
    /// Files Mode
    Files,
    /// Functions Mode
    Functions,
}

impl FromStr for Mode {
    type Err = String;

    fn from_str(mode: &str) -> std::result::Result<Self, Self::Err> {
        match mode {
            "files" => Ok(Mode::Files),
            "functions" => Ok(Mode::Functions),
            _ => Err(format!("{mode:?} is not a supported mode.")),
        }
    }
}

impl Mode {
    /// All modes.
    pub const fn all() -> &'static [&'static str] {
        &["files", "functions"]
    }

    /// Default mode.
    pub const fn default() -> &'static str {
        "files"
    }
}

/// Sort
#[derive(Copy, Debug, Clone, PartialEq, Eq, Hash)]
pub enum Sort {
    /// Wcc Plain
    WccPlain,
    /// Wcc Plain quantized
    WccQuantized,
    /// Crap
    Crap,
    /// Skunk
    Skunk,
}

impl FromStr for Sort {
    type Err = String;

    fn from_str(sort: &str) -> std::result::Result<Self, Self::Err> {
        match sort {
            "wcc_plain" => Ok(Sort::WccPlain),
            "wcc_quantized" => Ok(Sort::WccQuantized),
            "crap" => Ok(Sort::Crap),
            "skunk" => Ok(Sort::Skunk),
            _ => Err(format!("{sort:?} is not a supported metric.")),
        }
    }
}

impl Sort {
    /// All sorts.
    pub const fn all() -> &'static [&'static str] {
        &["wcc_plain", "wcc_quantized", "crap", "skunk"]
    }

    /// Default sort.
    pub const fn default() -> &'static str {
        "wcc_plain"
    }
}

pub(crate) trait Visit {
    fn get_metrics_from_space(
        space: &FuncSpace,
        covs: &[Value],
        metric: Complexity,
        coverage: Option<f64>,
        thresholds: &[f64],
    ) -> Result<(Metrics, (f64, f64))>;
}
pub(crate) struct Tree;

impl Visit for Tree {
    fn get_metrics_from_space(
        space: &FuncSpace,
        covs: &[Value],
        metric: Complexity,
        coverage: Option<f64>,
        thresholds: &[f64],
    ) -> Result<(Metrics, (f64, f64))> {
        let covdir = coverage.is_some();
        let (wcc_plain, sp_sum) = wcc_plain_function(space, covs, metric, covdir)?;
        let (wcc_quantized, sq_sum) = wcc_quantized_function(space, covs, metric, covdir)?;
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
pub(crate) fn read_files(files_path: &Path) -> Result<Vec<String>> {
    debug!("REading files in project folder: {:?}", files_path);
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

            if ext.is_some() && check_ext(ext.ok_or(Error::PathConversionError())?) {
                vec.push(path.display().to_string().replace('\\', "/"));
            }
        }
    }
    Ok(vec)
}

fn get_prefix<'a>(prefix: &'a str, to_check: &'a str) -> Result<String> {
    let mut s1 = prefix.split('/').collect::<Vec<&str>>();
    let mut s2 = to_check.split('/').collect::<Vec<&str>>();
    s1.pop();
    while let Some(s) = s2.pop() {
        if let Some(last) = s1.last() {
            if *last == s {
                s1.pop();
            }
        }
    }
    Ok(s1.join("/") + "/")
}

// This function read the content of the coveralls  json file obtain by using grcov
// Return a HashMap with all the files arrays of covered lines using the path to the file as key
pub(crate) fn read_json(file: String, prefix: &str) -> Result<HashMap<String, Vec<Value>>> {
    debug!("Reading coveralls json...");
    let val: Value = serde_json::from_str(file.as_str())?;
    let vec = val["source_files"]
        .as_array()
        .ok_or(Error::ReadingJSONError())?;
    let mut covs = HashMap::<String, Vec<Value>>::new();
    vec.iter().try_for_each(|x| -> Result<()> {
        let n = x["name"].as_str().ok_or(Error::PathConversionError())?;
        let prefix = get_prefix(prefix, n)?;
        let name = Path::new(&prefix).join(x["name"].as_str().ok_or(Error::PathConversionError())?);
        let value = x["coverage"]
            .as_array()
            .ok_or(Error::ConversionError())?
            .to_vec();
        covs.insert(name.display().to_string().replace('\\', "/"), value);
        Ok(())
    })?;
    Ok(covs)
}

// Struct used for covdir json parsing
#[derive(Clone, Default, Debug)]
#[allow(dead_code)]
pub(crate) struct Covdir {
    pub(crate) name: String,
    pub(crate) arr: Vec<Value>,
    pub(crate) coverage: f64,
}

// This function read the content of the coveralls  json file obtain by using grcov
// Return a HashMap with all the files arrays of covered lines using the path to the file as key
pub(crate) fn read_json_covdir(file: String, map_prefix: &str) -> Result<HashMap<String, Covdir>> {
    debug!("Reading covdir json...");
    let val: Map<String, Value> = serde_json::from_str(file.as_str())?;
    let mut res: HashMap<String, Covdir> = HashMap::<String, Covdir>::new();
    let mut stack = vec![(
        val["children"]
            .as_object()
            .ok_or(Error::ConversionError())?,
        "".to_string(),
    )];
    let covdir = Covdir {
        name: val["name"].as_str().ok_or(Error::ConversionError())?.into(),
        arr: vec![],
        coverage: val["coveragePercent"]
            .as_f64()
            .ok_or(Error::ConversionError())?,
    };
    res.insert("PROJECT_ROOT".into(), covdir);
    while let Some((val, prefix)) = stack.pop() {
        val.iter().try_for_each(|(key, value)| -> Result<()> {
            if value["children"].is_object() {
                if prefix.is_empty() {
                    stack.push((
                        value["children"]
                            .as_object()
                            .ok_or(Error::ConversionError())?,
                        prefix.to_owned() + key.as_str(),
                    ));
                } else {
                    let slash = if cfg!(windows) { "\\" } else { "/" };
                    stack.push((
                        value["children"]
                            .as_object()
                            .ok_or(Error::ConversionError())?,
                        prefix.to_owned() + slash + key.as_str(),
                    ));
                }
            }
            let name = value["name"]
                .as_str()
                .ok_or(Error::ConversionError())?
                .into();
            let path = Path::new(&name);
            let ext = path.extension();

            if ext.is_some() && check_ext(ext.ok_or(Error::PathConversionError())?) {
                let covdir = Covdir {
                    name,
                    arr: value["coverage"]
                        .as_array()
                        .ok_or(Error::ConversionError())?
                        .to_vec(),
                    coverage: value["coveragePercent"]
                        .as_f64()
                        .ok_or(Error::ConversionError())?,
                };
                let name_path = format!("{prefix}/{key}");
                let map_prefix = get_prefix(map_prefix, name_path.as_str())?;
                res.insert(map_prefix + name_path.as_str(), covdir);
            }
            Ok(())
        })?;
    }
    Ok(res)
}

// Get the code coverage in percentage between start and end
pub(crate) fn get_covered_lines(covs: &[Value], start: usize, end: usize) -> Result<(f64, f64)> {
    // Count the number of covered lines
    let (tot_lines, covered_lines) =
        covs.iter()
            .enumerate()
            .try_fold((0., 0.), |acc, (i, line)| -> Result<(f64, f64)> {
                let is_null = line.is_null();
                let sum;
                if !is_null && (start - 1..end).contains(&i) {
                    let cov = line.as_u64().ok_or(Error::ConversionError())?;
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
        .ok_or(Error::LanguageError())?;
    debug!("{:?} is written in {:?}", path.as_ref(), lang);
    let root = get_function_spaces(&lang, data, path.as_ref(), None).ok_or(Error::MetricsError())?;
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
                s.name.as_ref().ok_or(Error::PathConversionError())?,
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
pub(crate) fn check_complexity(
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
    values: JobComposer,
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

#[cfg(test)]
mod tests {

    use super::*;
    const JSON: &str = "./data/data.json";
    const PREFIX: &str = "../rust-data-structures-main/";
    const SIMPLE: &str = "../rust-data-structures-main/data/simple_main.rs";
    const MAIN: &str = "../rust-data-structures-main/data/main.rs";

    #[test]
    fn test_read_json() {
        let file = fs::read_to_string(JSON).unwrap();
        let covs = read_json(file, PREFIX).unwrap();
        assert!(covs.contains_key(SIMPLE));
        assert!(covs.contains_key(MAIN));
        let vec = covs.get(SIMPLE).unwrap();
        assert_eq!(vec.len(), 12);
        let vec_main = covs.get(MAIN).unwrap();
        assert_eq!(vec_main.len(), 9);
        let value = vec.get(6).unwrap();
        assert_eq!(value, 2);
        let value_null = vec.get(1).unwrap();
        assert!(value_null.is_null());
    }
}
