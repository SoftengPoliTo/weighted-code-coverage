#![deny(missing_docs, unsafe_code)]

//! The `weighted-code-coverage` tool implements various
//! weighted code coverage algorithms, identifying code parts
//! which are both complex and with low coverage
//! according to the following complexity metrics:
//!
//! - Cyclomatic
//! - Cognitive
//!
//! The tool implements the following algorithms:
//!
//! - Wcc
//! - Crap
//! - SKUNK

mod concurrent;
mod error;
mod grcov;
mod metrics;
mod output;

use std::{
    ffi::OsStr,
    fmt, fs,
    io::ErrorKind,
    path::{Path, PathBuf},
    str::FromStr,
    sync::Mutex,
};

use concurrent::{Grcov, Wcc, WccConcurrent, WccOutput};
use error::{Error, Result};
use grcov::{covdir::Covdir, coveralls::Coveralls};
use metrics::MetricsThresholds;
use output::{HtmlPrinter, JsonPrinter, WccPrinter};
use serde::Serialize;

const JSON_OUTPUT_PATH: &str = "wcc.json";

#[derive(Debug)]
struct Parameters<P: AsRef<Path>> {
    n_threads: usize,
    grcov_format: GrcovFormat<PathBuf>,
    mode: Mode,
    thresholds: MetricsThresholds,
    sort_by: Sort,
    json_path: PathBuf,
    html_path: Option<P>,
}

impl<P: AsRef<Path>> Default for Parameters<P> {
    fn default() -> Self {
        Self {
            thresholds: MetricsThresholds::default(),
            n_threads: (rayon::current_num_threads() - 1).max(1),
            grcov_format: GrcovFormat::default(),
            mode: Mode::default(),
            sort_by: Sort::default(),
            json_path: PathBuf::from(JSON_OUTPUT_PATH),
            html_path: Option::default(),
        }
    }
}

/// Run weighted code coverage for a project.
///
/// If no parameters are set, the runner uses:
/// * *cyclomatic* with thresholds values *[35.0, 1.5, 35.0, 30.0]* as a default metric.
/// * *maximum number of threads - 1* as default number of threads.
/// * *coveralls* as default format for the input grcov json file.
/// * *files* as default analysis mode.
/// * *wcc plain* as default metric that will be used to sort the output.
#[derive(Debug)]
pub struct WccRunner<P: AsRef<Path>>(Parameters<P>);

impl<P: AsRef<Path>> WccRunner<P> {
    /// Creates a new `WccRunner` instance.
    pub fn new() -> Self {
        Self(Parameters::default())
    }

    /// Sets thresholds values that will be used.
    pub fn thresholds(mut self, thresholds: Thresholds) -> Self {
        self.0.thresholds = thresholds.into();
        self
    }

    /// Sets number of threads that will be used.
    pub fn n_threads(mut self, n_threads: usize) -> Self {
        self.0.n_threads = (rayon::current_num_threads() - 1).max(1).min(n_threads);
        self
    }

    /// Sets format of the input grcov json file and its path.
    pub fn grcov_format<T: Into<PathBuf>>(mut self, grcov_format: GrcovFormat<T>) -> Self {
        self.0.grcov_format = grcov_format.into();
        self
    }

    /// Sets mode that will be used for the analysis.
    pub fn mode(mut self, mode: Mode) -> Self {
        self.0.mode = mode;
        self
    }

    /// Sets the metric that will be used to sort the output.
    pub fn sort_by(mut self, sort_by: Sort) -> Self {
        self.0.sort_by = sort_by;
        self
    }

    /// Sets the path of the json output.
    pub fn json_path<T: Into<PathBuf>>(mut self, json_path: T) -> Self {
        self.0.json_path = json_path.into();
        self
    }

    /// Sets the path of the html output.
    pub fn html_path(mut self, html_path: Option<P>) -> Self {
        self.0.html_path = html_path;
        self
    }

    /// Runs the weighted code coverage runner.
    pub fn run<'a, T: AsRef<Path> + Sync + 'a>(self, project_path: T) -> Result<WccOutput> {
        if let Some(extension) = self.0.json_path.extension() {
            if extension.to_ascii_lowercase() != "json" {
                return Err(Error::OutputPath("Json output path must be a json file"));
            }
        }

        if let Some(path) = &self.0.html_path {
            if !path.as_ref().is_dir() {
                return Err(Error::OutputPath("Html output path must be a directory"));
            }
        }

        let files = read_files(project_path.as_ref())?;
        let grcov = self.get_grcov(project_path.as_ref())?;

        let wcc_output = Wcc {
            project_path: project_path.as_ref(),
            files: &files,
            mode: self.0.mode,
            grcov,
            thresholds: self.0.thresholds,
            files_metrics: Mutex::new(Vec::new()),
            ignored_files: Mutex::new(Vec::new()),
            sort_by: self.0.sort_by,
        }
        .run(self.0.n_threads)?;

        self.print(&wcc_output, project_path.as_ref())?;

        Ok(wcc_output)
    }

    fn get_grcov(&self, project_path: &Path) -> Result<Grcov> {
        let grcov = match &self.0.grcov_format {
            GrcovFormat::Coveralls(coveralls_path) => {
                Grcov::Coveralls(Coveralls::new(coveralls_path.as_ref(), project_path)?)
            }
            GrcovFormat::Covdir(covdir_path) => {
                Grcov::Covdir(Covdir::new(covdir_path.as_ref(), project_path)?)
            }
        };

        Ok(grcov)
    }

    fn print(&self, wcc_output: &WccOutput, project_path: &Path) -> Result<()> {
        JsonPrinter {
            project_path,
            wcc_output,
            output_path: self.0.json_path.as_ref(),
            mode: self.0.mode,
            thresholds: self.0.thresholds,
        }
        .print()?;

        if let Some(html_path) = &self.0.html_path {
            HtmlPrinter {
                wcc_output,
                output_path: html_path.as_ref(),
                mode: self.0.mode,
                thresholds: self.0.thresholds,
            }
            .print()?;
        }

        Ok(())
    }
}

impl<P: AsRef<Path>> Default for WccRunner<P> {
    fn default() -> Self {
        Self::new()
    }
}

// Checks if the file extension is valid.
#[inline]
fn valid_extension(ext: &OsStr) -> bool {
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

// Returns the list of project source files.
#[inline]
fn read_files(project_path: &Path) -> Result<Vec<PathBuf>> {
    let mut files = vec![];
    let mut stack = vec![project_path.to_path_buf()];

    while let Some(path) = stack.pop() {
        if path.is_dir() {
            // Skip ./target directory and all its subdirectories.
            if let Some(dir_name) = path.file_name().and_then(|n| n.to_str()) {
                if dir_name.contains("target") {
                    continue;
                }
            }
            let mut entries = fs::read_dir(&path)?;
            entries.try_for_each(|entry| -> Result<()> {
                stack.push(entry?.path());
                Ok(())
            })?;
        } else if let Some(extension) = path.extension() {
            if valid_extension(extension) {
                files.push(PathBuf::from(path.to_string_lossy().replace('\\', "/")));
            }
        }
    }

    Ok(files)
}

/// Availabe grcov json file formats.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum GrcovFormat<P: Into<PathBuf>> {
    /// Coveralls.
    Coveralls(P),
    /// Covdir.
    Covdir(P),
}

impl Default for GrcovFormat<PathBuf> {
    fn default() -> Self {
        Self::Coveralls(PathBuf::from("coveralls.json"))
    }
}

impl<P: Into<PathBuf>> GrcovFormat<P> {
    /// Parses cli coveralls argument.
    pub fn coveralls_parser(
        coveralls_file: &str,
    ) -> std::result::Result<GrcovFormat<PathBuf>, Box<std::io::Error>> {
        Ok(GrcovFormat::Coveralls(PathBuf::from(coveralls_file)))
    }

    /// Parses cli covdir argument.
    pub fn covdir_parser(
        covdir_file: &str,
    ) -> std::result::Result<GrcovFormat<PathBuf>, Box<std::io::Error>> {
        Ok(GrcovFormat::Covdir(PathBuf::from(covdir_file)))
    }

    fn into(self) -> GrcovFormat<PathBuf> {
        match self {
            GrcovFormat::Coveralls(coveralls_path) => GrcovFormat::Coveralls(coveralls_path.into()),
            GrcovFormat::Covdir(covdir_path) => GrcovFormat::Covdir(covdir_path.into()),
        }
    }
}

/// Complexity Metrics.
#[derive(Copy, Debug, Default, Clone, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Complexity {
    /// Cyclomatic metric.
    #[default]
    Cyclomatic,
    /// Cognitive metric.
    Cognitive,
}

/// Thresholds.
#[derive(Debug, Clone, Copy, Serialize)]
pub struct Thresholds {
    wcc: f64,
    cyclomatic_complexity: f64,
    cognitive_complexity: f64,
}

impl fmt::Display for Thresholds {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "{},{},{}",
            self.wcc, self.cyclomatic_complexity, self.cognitive_complexity
        )
    }
}

impl Default for Thresholds {
    fn default() -> Self {
        Self {
            wcc: 60.0,
            cyclomatic_complexity: 10.0,
            cognitive_complexity: 10.0,
        }
    }
}

impl FromStr for Thresholds {
    type Err = std::io::Error;

    fn from_str(thresholds: &str) -> std::result::Result<Self, Self::Err> {
        let mut iter = thresholds.split(',').filter_map(|s| s.parse::<f64>().ok());

        Ok(Self {
            wcc: iter.next().ok_or(std::io::Error::new(
                ErrorKind::InvalidInput,
                format!("Missing or invalid wcc in thresholds: {}", thresholds),
            ))?,
            cyclomatic_complexity: iter.next().ok_or(std::io::Error::new(
                ErrorKind::InvalidInput,
                format!(
                    "Missing or invalid cyclomatic complexity in thresholds: {}",
                    thresholds
                ),
            ))?,
            cognitive_complexity: iter.next().ok_or(std::io::Error::new(
                ErrorKind::InvalidInput,
                format!(
                    "Missing or invalid cognitive complexity in thresholds: {}",
                    thresholds
                ),
            ))?,
        })
    }
}

/// Mode.
#[derive(Copy, Debug, Default, Clone, PartialEq, Eq, Hash, Serialize)]
pub enum Mode {
    /// Files Mode.
    #[default]
    Files,
    /// Functions Mode.
    Functions,
}

impl Mode {
    /// All `Mode` options.
    pub const fn all() -> &'static [&'static str] {
        &["files", "functions"]
    }

    /// Default `Mode` option.
    pub const fn default_value() -> &'static str {
        "files"
    }
}

impl fmt::Display for Mode {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let s = match self {
            Self::Files => "files",
            Self::Functions => "functions",
        };
        s.fmt(f)
    }
}

impl FromStr for Mode {
    type Err = std::io::Error;

    fn from_str(mode: &str) -> std::result::Result<Self, Self::Err> {
        match mode {
            "files" => Ok(Mode::Files),
            "functions" => Ok(Mode::Functions),
            _ => Err(std::io::Error::new(
                ErrorKind::Other,
                format!("{mode:?} is not a supported mode."),
            )),
        }
    }
}

/// Sort.
#[derive(Copy, Debug, Default, Clone, PartialEq, Eq, Hash)]
pub enum Sort {
    /// Wcc.
    #[default]
    Wcc,
    /// Crap.
    Crap,
    /// Skunk.
    Skunk,
}

impl Sort {
    /// All `Sort` options.
    pub const fn all() -> &'static [&'static str] {
        &["wcc", "crap", "skunk"]
    }

    /// Default `Sort` option.
    pub const fn default_value() -> &'static str {
        "wcc"
    }
}

impl fmt::Display for Sort {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let s = match self {
            Self::Wcc => "wcc",
            Self::Crap => "crap",
            Self::Skunk => "skunk",
        };
        s.fmt(f)
    }
}

impl FromStr for Sort {
    type Err = std::io::Error;

    fn from_str(sort: &str) -> std::result::Result<Self, Self::Err> {
        match sort {
            "wcc" => Ok(Sort::Wcc),
            "crap" => Ok(Sort::Crap),
            "skunk" => Ok(Sort::Skunk),
            _ => Err(std::io::Error::new(
                ErrorKind::Other,
                format!("{sort:?} is not a supported metric."),
            )),
        }
    }
}
