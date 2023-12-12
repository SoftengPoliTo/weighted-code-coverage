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
//! - WCC plain
//! - WCC quantized
//! - Crap
//! - SKUNK

mod concurrent;
mod error;
mod grcov;
mod metrics;
mod output;
mod utility;

use std::{
    fmt, fs,
    io::ErrorKind,
    path::{Path, PathBuf},
    str::FromStr,
};

use concurrent::{
    files::{CovdirFilesWcc, CoverallsFilesWcc, FileMetrics},
    functions::{CovdirFunctionsWcc, CoverallsFunctionsWcc, FunctionMetrics, RootMetrics},
    Wcc,
};

use error::{Error, Result};
use grcov::{covdir::Covdir, coveralls::Coveralls};
use output::{
    get_metrics_output, get_metrics_output_function, print_metrics_to_csv,
    print_metrics_to_csv_function, print_metrics_to_html, print_metrics_to_html_function,
    print_metrics_to_json, print_metrics_to_json_function,
};
use utility::{chunk_vector, get_prefix, read_files};

#[derive(Debug)]
struct Parameters<P: AsRef<Path> + Default> {
    complexity: (Complexity, Thresholds),
    n_threads: usize,
    json_format: JsonFormat,
    mode: Mode,
    sort_by: Sort,
    output_format: OutputFormat,
    output_path: P,
}

impl<P: AsRef<Path> + Default> Default for Parameters<P> {
    fn default() -> Self {
        Self {
            complexity: (
                Complexity::default(),
                Thresholds(vec![35.0, 1.5, 35.0, 30.0]),
            ),
            n_threads: (rayon::current_num_threads() - 1).max(1),
            json_format: JsonFormat::Coveralls(PathBuf::from("")),
            mode: Mode::default(),
            sort_by: Sort::default(),
            output_format: OutputFormat::default(),
            output_path: P::default(),
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
pub struct WccRunner<P: AsRef<Path> + Default>(Parameters<P>);

impl<P: AsRef<Path> + Default> WccRunner<P> {
    /// Creates a new `WccRunner` instance.
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets complexity metric and thresholds values that will be used.
    pub fn complexity(mut self, complexity: (Complexity, Thresholds)) -> Self {
        self.0.complexity = complexity;
        self
    }

    /// Sets number of threads that will be used.
    pub fn n_threads(mut self, n_threads: usize) -> Self {
        self.0.n_threads = n_threads;
        self
    }

    /// Sets format of the input grcov json file and its path.
    pub fn json_format(mut self, json_format: JsonFormat) -> Self {
        self.0.json_format = json_format;
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
    pub fn output_path(mut self, output_path: P) -> Self {
        self.0.output_path = output_path;
        self
    }

    /// Runs the weighted code coverage runner.
    pub fn run<T: AsRef<Path>>(self, project_path: T) -> Result<()> {
        if self.0.complexity.1 .0.len() != 4 {
            return Err(Error::Thresholds);
        }

        let files = read_files(&project_path)?;
        let grcov_json = fs::read_to_string(self.0.json_format.file_path())?;
        let prefix = get_prefix(&project_path)?;
        let chunks = chunk_vector(files, self.0.n_threads);

        match self.0.mode {
            Mode::Files => {
                let files_metrics =
                    self.get_files_wcc_output(grcov_json, prefix, chunks, &project_path)?;
                self.print_files_metrics(files_metrics, &project_path)?;
            }
            Mode::Functions => {
                let functions_metrics =
                    self.get_functions_wcc_output(grcov_json, prefix, chunks, &project_path)?;
                self.print_functions_metrics(functions_metrics, project_path)?;
            }
        };

        Ok(())
    }

    fn get_files_wcc_output<T: AsRef<Path>>(
        &self,
        grcov_json: String,
        prefix: usize,
        chunks: Vec<Vec<String>>,
        project_path: T,
    ) -> Result<FilesWccOutput> {
        let output = match self.0.json_format {
            JsonFormat::Coveralls(_) => CoverallsFilesWcc::new(
                chunks,
                Coveralls::new(grcov_json, project_path)?,
                self.0.complexity.0,
                prefix,
                self.0.complexity.1 .0.to_owned(),
                self.0.sort_by,
            )
            .run(self.0.n_threads)?,
            JsonFormat::Covdir(_) => CovdirFilesWcc::new(
                chunks,
                Covdir::new(grcov_json, project_path)?,
                self.0.complexity.0,
                prefix,
                self.0.complexity.1 .0.to_owned(),
                self.0.sort_by,
            )
            .run(self.0.n_threads)?,
        };

        Ok(output)
    }

    fn get_functions_wcc_output<T: AsRef<Path>>(
        &self,
        grcov_json: String,
        prefix: usize,
        chunks: Vec<Vec<String>>,
        project_path: T,
    ) -> Result<FunctionsWccOutput> {
        let output = match self.0.json_format {
            JsonFormat::Coveralls(_) => CoverallsFunctionsWcc::new(
                chunks,
                Coveralls::new(grcov_json, project_path)?,
                self.0.complexity.0,
                prefix,
                self.0.complexity.1 .0.to_owned(),
                self.0.sort_by,
            )
            .run(self.0.n_threads)?,
            JsonFormat::Covdir(_) => CovdirFunctionsWcc::new(
                chunks,
                Covdir::new(grcov_json, project_path)?,
                self.0.complexity.0,
                prefix,
                self.0.complexity.1 .0.to_owned(),
                self.0.sort_by,
            )
            .run(self.0.n_threads)?,
        };

        Ok(output)
    }

    fn print_files_metrics<T: AsRef<Path>>(
        &self,
        files_metrics: FilesWccOutput,
        project_path: T,
    ) -> Result<()> {
        let (metrics, files_ignored, complex_files, project_coverage) = files_metrics;
        match self.0.output_format {
            OutputFormat::Json => print_metrics_to_json(
                &metrics,
                &files_ignored,
                self.0.output_path.as_ref(),
                project_path.as_ref(),
                project_coverage,
                self.0.sort_by,
            )?,
            OutputFormat::Csv => print_metrics_to_csv(
                &metrics,
                &files_ignored,
                self.0.output_path.as_ref(),
                project_coverage,
                self.0.sort_by,
            )?,
            OutputFormat::Html => print_metrics_to_html(
                &metrics,
                &files_ignored,
                self.0.output_path.as_ref(),
                project_path.as_ref(),
                project_coverage,
                self.0.sort_by,
            )?,
        };
        get_metrics_output(&metrics, &files_ignored, &complex_files);

        Ok(())
    }

    fn print_functions_metrics<T: AsRef<Path>>(
        &self,
        functions_metrics: FunctionsWccOutput,
        project_path: T,
    ) -> Result<()> {
        let (metrics, files_ignored, complex_files, project_coverage) = functions_metrics;
        match self.0.output_format {
            OutputFormat::Json => print_metrics_to_json_function(
                &metrics,
                &files_ignored,
                self.0.output_path.as_ref(),
                project_path.as_ref(),
                project_coverage,
                self.0.sort_by,
            )?,
            OutputFormat::Csv => print_metrics_to_csv_function(
                &metrics,
                &files_ignored,
                self.0.output_path.as_ref(),
                project_coverage,
                self.0.sort_by,
            )?,
            OutputFormat::Html => print_metrics_to_html_function(
                &metrics,
                &files_ignored,
                self.0.output_path.as_ref(),
                project_path.as_ref(),
                project_coverage,
                self.0.sort_by,
            )?,
        };
        get_metrics_output_function(&metrics, &files_ignored, &complex_files);

        Ok(())
    }
}

type FilesWccOutput = (Vec<FileMetrics>, Vec<String>, Vec<FileMetrics>, f64);
type FunctionsWccOutput = (Vec<RootMetrics>, Vec<String>, Vec<FunctionMetrics>, f64);

/// Availabe grcov json file formats
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum JsonFormat {
    /// Coveralls.
    Coveralls(PathBuf),
    /// Covdir.
    Covdir(PathBuf),
}

impl JsonFormat {
    /// Parses cli coveralls argument.
    pub fn coveralls_parser(
        coveralls_file: &str,
    ) -> std::result::Result<JsonFormat, Box<std::io::Error>> {
        Ok(JsonFormat::Coveralls(PathBuf::from(coveralls_file)))
    }

    /// Parses cli covdir argument.
    pub fn covdir_parser(
        covdir_file: &str,
    ) -> std::result::Result<JsonFormat, Box<std::io::Error>> {
        Ok(JsonFormat::Covdir(PathBuf::from(covdir_file)))
    }

    fn file_path(&self) -> &PathBuf {
        match self {
            Self::Coveralls(coveralls_file_path) => coveralls_file_path,
            Self::Covdir(covdir_file_path) => covdir_file_path,
        }
    }
}

/// Complexity Metrics
#[derive(Copy, Debug, Default, Clone, PartialEq, Eq, Hash)]
pub enum Complexity {
    /// Cyclomatic metric.
    #[default]
    Cyclomatic,
    /// Cognitive metric.
    Cognitive,
}

impl fmt::Display for Complexity {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::Cyclomatic => write!(f, "cyclomatic"),
            Self::Cognitive => write!(f, "cognitive"),
        }
    }
}

impl FromStr for Complexity {
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

impl Complexity {
    /// All complexity formats.
    pub const fn all() -> &'static [&'static str] {
        &["cyclomatic", "cognitive"]
    }

    /// Default complexity format.
    pub const fn default_value() -> &'static str {
        "cyclomatic"
    }
}

/// Thresholds
#[derive(Debug, Clone, Default)]
pub struct Thresholds(Vec<f64>);

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
#[derive(Copy, Debug, Default, Clone, PartialEq, Eq, Hash)]
pub enum Mode {
    /// Files Mode
    #[default]
    Files,
    /// Functions Mode
    Functions,
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

/// Sort
#[derive(Copy, Debug, Default, Clone, PartialEq, Eq, Hash)]
pub enum Sort {
    /// Wcc Plain
    #[default]
    WccPlain,
    /// Wcc Plain quantized
    WccQuantized,
    /// Crap
    Crap,
    /// Skunk
    Skunk,
}

impl FromStr for Sort {
    type Err = std::io::Error;

    fn from_str(sort: &str) -> std::result::Result<Self, Self::Err> {
        match sort {
            "wcc_plain" => Ok(Sort::WccPlain),
            "wcc_quantized" => Ok(Sort::WccQuantized),
            "crap" => Ok(Sort::Crap),
            "skunk" => Ok(Sort::Skunk),
            _ => Err(std::io::Error::new(
                ErrorKind::Other,
                format!("{sort:?} is not a supported metric."),
            )),
        }
    }
}

impl Sort {
    /// All sorts.
    pub const fn all() -> &'static [&'static str] {
        &["wcc_plain", "wcc_quantized", "crap", "skunk"]
    }

    /// Default sort.
    pub const fn default_value() -> &'static str {
        "wcc_plain"
    }
}

/// Available output formats
#[derive(Debug, Clone, PartialEq, Default, Eq, Hash)]
pub enum OutputFormat {
    /// JSON
    #[default]
    Json,
    /// CSV
    Csv,
    /// HTML
    Html,
}

impl FromStr for OutputFormat {
    type Err = std::io::Error;

    fn from_str(output_format: &str) -> std::result::Result<Self, Self::Err> {
        match output_format {
            "json" => Ok(OutputFormat::Json),
            "csv" => Ok(OutputFormat::Csv),
            "html" => Ok(OutputFormat::Html),
            _ => Err(std::io::Error::new(
                ErrorKind::Other,
                format!("{output_format:?} is not a supported output format."),
            )),
        }
    }
}

impl OutputFormat {
    /// All output formats.
    pub const fn all() -> &'static [&'static str] {
        &["json", "csv", "html"]
    }

    /// Default output format.
    pub const fn default_value() -> &'static str {
        "json"
    }
}
