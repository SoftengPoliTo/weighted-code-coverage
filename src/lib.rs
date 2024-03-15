#![deny(missing_docs, unsafe_code)]

//! The `weighted-code-coverage` tool implements various
//! weighted code coverage algorithms, identifying code parts
//! which are both complex and without any code coverage
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
};

use concurrent::{Grcov, Wcc, WccConcurrent, WccOutput};
use error::{Error, Result};
use grcov::{covdir::Covdir, coveralls::Coveralls};
use output::{HtmlPrinter, JsonPrinter, WccPrinter};
use serde::Serialize;

#[derive(Debug)]
struct Parameters<A: AsRef<Path> + Default, B: AsRef<Path> + Default> {
    complexity: Complexity,
    n_threads: usize,
    grcov_format: GrcovFormat<A>,
    mode: Mode,
    sort_by: Sort,
    output_format: OutputFormat,
    output_path: Option<B>,
}

impl<A: AsRef<Path> + Default, B: AsRef<Path> + Default> Default for Parameters<A, B> {
    fn default() -> Self {
        Self {
            complexity: Complexity::default(),
            n_threads: (rayon::current_num_threads() - 1).max(1),
            grcov_format: GrcovFormat::Coveralls(A::default()),
            mode: Mode::default(),
            sort_by: Sort::default(),
            output_format: OutputFormat::default(),
            output_path: None,
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
#[derive(Debug, Default)]
pub struct WccRunner<A: AsRef<Path> + Default, B: AsRef<Path> + Default>(Parameters<A, B>);

impl<A: AsRef<Path> + Default, B: AsRef<Path> + Default> WccRunner<A, B> {
    /// Creates a new `WccRunner` instance.
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets complexity metric and thresholds values that will be used.
    pub fn complexity(mut self, complexity: Complexity) -> Self {
        self.0.complexity = complexity;
        self
    }

    /// Sets number of threads that will be used.
    pub fn n_threads(mut self, n_threads: usize) -> Self {
        self.0.n_threads = (rayon::current_num_threads() - 1).max(1).min(n_threads);
        self
    }

    /// Sets format of the input grcov json file and its path.
    pub fn grcov_format(mut self, grcov_format: GrcovFormat<A>) -> Self {
        self.0.grcov_format = grcov_format;
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

    /// Sets the format of the output file.
    pub fn output_format(mut self, output_format: OutputFormat) -> Self {
        self.0.output_format = output_format;
        self
    }

    /// Sets the path of the output file.
    pub fn output_path(mut self, output_path: B) -> Self {
        self.0.output_path = Some(output_path);
        self
    }

    /// Runs the weighted code coverage runner.
    pub fn run<'a, P: AsRef<Path> + Sync + 'a>(self, project_path: P) -> Result<WccOutput> {
        if self.0.complexity.thresholds.0.len() != 3 {
            return Err(Error::Thresholds);
        }

        let files = read_files(project_path.as_ref())?;
        let grcov = self.get_grcov(project_path.as_ref())?;

        let wcc_output = Wcc::new(
            project_path.as_ref(),
            &files,
            self.0.mode,
            grcov,
            self.0.complexity.complexity_type,
            &self.0.complexity.thresholds.0,
            self.0.sort_by,
        )
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
        if let Some(output_path) = &self.0.output_path {
            match self.0.output_format {
                OutputFormat::Json => JsonPrinter::new(
                    project_path,
                    wcc_output,
                    output_path.as_ref(),
                    self.0.mode,
                    &self.0.complexity.complexity_type,
                )
                .print()?,
                OutputFormat::Html => HtmlPrinter::new(
                    wcc_output,
                    output_path.as_ref(),
                    self.0.mode,
                    &self.0.complexity,
                )
                .print()?,
            };
        }

        Ok(())
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
fn read_files(project_path: &Path) -> Result<Vec<PathBuf>> {
    let mut files = vec![];
    let mut stack = vec![project_path.to_path_buf()];

    while let Some(path) = stack.pop() {
        if path.is_dir() {
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

/// Availabe grcov json file formats
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum GrcovFormat<P: AsRef<Path>> {
    /// Coveralls.
    Coveralls(P),
    /// Covdir.
    Covdir(P),
}

impl<P: AsRef<Path>> GrcovFormat<P> {
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
}

/// Complexity Metrics.
#[derive(Copy, Debug, Default, Clone, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ComplexityType {
    /// Cyclomatic metric.
    #[default]
    Cyclomatic,
    /// Cognitive metric.
    Cognitive,
}

impl fmt::Display for ComplexityType {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let s = match self {
            Self::Cyclomatic => "cyclomatic",
            Self::Cognitive => "cognitive",
        };
        s.fmt(f)
    }
}

impl FromStr for ComplexityType {
    type Err = std::io::Error;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s {
            "cyclomatic" => Ok(Self::Cyclomatic),
            "cognitive" => Ok(Self::Cognitive),
            _ => Err(std::io::Error::new(
                ErrorKind::Other,
                format!("Unknown complexity metric: {s}"),
            )),
        }
    }
}

impl ComplexityType {
    /// All complexity formats.
    pub const fn all() -> &'static [&'static str] {
        &["cyclomatic", "cognitive"]
    }

    /// Default complexity format.
    pub const fn default_value() -> &'static str {
        "cyclomatic"
    }
}

/// Complexity metric with thresholds.
#[derive(Debug, Clone, Default)]
pub struct Complexity {
    /// Complexity type.
    pub complexity_type: ComplexityType,
    /// Thresholds.
    pub thresholds: Thresholds,
}

impl Complexity {
    /// Parses cli Complexity argument.
    pub fn parser(s: &str) -> std::result::Result<Self, Box<std::io::Error>> {
        let pos = s.find(':').ok_or_else(|| {
            std::io::Error::new(
                ErrorKind::InvalidInput,
                format!("invalid KEY:value: no `:` found in `{s}`"),
            )
        })?;

        Ok(Self {
            complexity_type: s[..pos].parse()?,
            thresholds: s[pos + 1..].parse()?,
        })
    }
}

impl fmt::Display for Complexity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        format!("{}:{}", self.complexity_type, self.thresholds).fmt(f)
    }
}

/// Thresholds
#[derive(Debug, Clone, Serialize)]
pub struct Thresholds(pub Vec<f64>);

impl fmt::Display for Thresholds {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let s: String = self
            .0
            .iter()
            .map(|&num| num.to_string())
            .collect::<Vec<_>>()
            .join(",");
        write!(f, "{}", s)
    }
}

impl Default for Thresholds {
    fn default() -> Self {
        Self(vec![60.0, 30.0, 30.0])
    }
}

impl FromStr for Thresholds {
    type Err = std::io::Error;

    fn from_str(thresholds: &str) -> std::result::Result<Self, Self::Err> {
        let parsed_thresholds: std::result::Result<Vec<f64>, _> = thresholds
            .split(',')
            .map(|t| t.trim().parse::<f64>())
            .collect();

        match parsed_thresholds {
            Ok(values) => Ok(Thresholds(values)),
            Err(_) => Err(std::io::Error::new(
                ErrorKind::Other,
                format!("{thresholds:?} format is invalid."),
            )),
        }
    }
}

/// Mode
#[derive(Copy, Debug, Default, Clone, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Mode {
    /// Files Mode
    #[default]
    Files,
    /// Functions Mode
    Functions,
}

impl Mode {
    /// All modes.
    pub const fn all() -> &'static [&'static str] {
        &["files", "functions"]
    }

    /// Default mode.
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

/// Sort
#[derive(Copy, Debug, Default, Clone, PartialEq, Eq, Hash)]
pub enum Sort {
    /// Wcc
    #[default]
    Wcc,
    /// Crap
    Crap,
    /// Skunk
    Skunk,
}

impl Sort {
    /// All sorts.
    pub const fn all() -> &'static [&'static str] {
        &["wcc", "crap", "skunk"]
    }

    /// Default sort.
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

/// Available output formats
#[derive(Debug, Clone, PartialEq, Default, Eq, Hash)]
pub enum OutputFormat {
    /// JSON
    #[default]
    Json,
    /// HTML
    Html,
}

impl OutputFormat {
    /// All output formats.
    pub const fn all() -> &'static [&'static str] {
        &["json", "html"]
    }

    /// Default output format.
    pub const fn default_value() -> &'static str {
        "json"
    }
}

impl fmt::Display for OutputFormat {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let s = match self {
            Self::Json => "json",
            Self::Html => "html",
        };
        s.fmt(f)
    }
}

impl FromStr for OutputFormat {
    type Err = std::io::Error;

    fn from_str(output_format: &str) -> std::result::Result<Self, Self::Err> {
        match output_format {
            "json" => Ok(OutputFormat::Json),
            "html" => Ok(OutputFormat::Html),
            _ => Err(std::io::Error::new(
                ErrorKind::Other,
                format!("{output_format:?} is not a supported output format."),
            )),
        }
    }
}
