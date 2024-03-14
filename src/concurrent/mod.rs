pub(crate) mod files;
pub(crate) mod functions;

use std::{
    path::{Path, PathBuf},
    sync::Mutex,
};

use crossbeam::channel::{Receiver, Sender};
use rayon::iter::{IntoParallelIterator, ParallelIterator};
use rust_code_analysis::{FuncSpace, SpaceKind};
use serde::Serialize;

use crate::{
    error::{Error, Result},
    grcov::{covdir::Covdir, coveralls::Coveralls},
    metrics::{
        crap::crap, get_root, round_sd,
        skunk::skunk,
        wcc::{wcc_file, wcc_func_space, WccFuncSpace}, LinesMetrics,
    },
    Complexity, Mode, Sort,
};

use self::{files::FileMetrics, functions::FunctionMetrics};

// Defines a framework for a *producers-consumers-composer* pattern
// used to compute weighted code coverage.
pub(crate) trait WccConcurrent {
    // Item sent from `producer` to `consumer`.
    type ProducerItem: Sync + Send;

    // Item sent from `consumer` to `composer`.
    type ConsumerItem: Sync + Send;

    // Output returned by the `composer`.
    type Output: Sync + Send;

    // Sends items to the `consumer`.
    //
    // * `sender` - `Sender` of the channel between `producer` and `consumer`.
    fn producer(&self, sender: Sender<Self::ProducerItem>) -> Result<()>;

    // Receivs items from the `producer`, processes them, and sends the results
    // to the `composer`.
    //
    // * `receiver` - `Receiver` of the channel between `producer` and `consumer`.
    // * `sender` - `Sender` of the channel between `consumer` and `composer`.
    fn consumer(
        &self,
        receiver: Receiver<Self::ProducerItem>,
        sender: Sender<Self::ConsumerItem>,
    ) -> Result<()>;

    // Receivs items from the `consumer`, computes an `Output`, and returns it.
    //
    // * `receiver` - `Receiver` of the channel between `consumer` and `composer`.
    fn composer(&self, receiver: Receiver<Self::ConsumerItem>) -> Result<Self::Output>;

    // Executes the *producers-consumers-composer* pattern.
    fn run(&self, n_threads: usize) -> Result<Self::Output>
    where
        Self: Sync,
    {
        let (producer_to_consumer_sender, producer_to_consumer_receiver) =
            crossbeam::channel::bounded(n_threads);
        let (consumer_to_composer_sender, consumer_to_composer_receiver) =
            crossbeam::channel::bounded(n_threads);

        match crossbeam::thread::scope(|scope| {
            // Producer
            scope.spawn(|_| self.producer(producer_to_consumer_sender));

            // Composer
            let composer = scope.spawn(|_| self.composer(consumer_to_composer_receiver));

            // Consumer
            (0..n_threads).into_par_iter().try_for_each(move |_| {
                self.consumer(
                    producer_to_consumer_receiver.clone(),
                    consumer_to_composer_sender.clone(),
                )
            })?;

            composer.join()?
        }) {
            Ok(output) => output,
            Err(e) => Err(e.into()),
        }
    }
}

pub(crate) enum Grcov {
    Coveralls(Coveralls),
    Covdir(Covdir),
}

impl Grcov {
    fn get_lines_coverage<P: AsRef<Path>>(&self, file: P) -> Option<&[Option<i32>]> {
        match self {
            Grcov::Coveralls(coveralls) => Some(&coveralls.0.get(file.as_ref())?.coverage),
            Grcov::Covdir(covdir) => Some(&covdir.source_files.get(file.as_ref())?.coverage),
        }
    }

    fn get_file_name<A: AsRef<Path>, B: AsRef<Path>>(
        &self,
        file: A,
        project_path: B,
    ) -> Result<String> {
        match self {
            Grcov::Coveralls(coveralls) => Ok(coveralls
                .0
                .get(file.as_ref())
                .ok_or(Error::HashMap)?
                .name
                .to_str()
                .ok_or(Error::Conversion)?
                .to_string()),
            Grcov::Covdir(_) => Ok(file
                .as_ref()
                .to_path_buf()
                .strip_prefix(project_path)?
                .to_str()
                .ok_or(Error::Conversion)?
                .to_string()),
        }
    }
}

/// Metrics values.
#[derive(Debug, Clone, Serialize)]
pub struct Metrics {
    pub wcc: f64,
    pub crap: f64,
    pub skunk: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_complex: Option<bool>,
    pub coverage: f64,
    pub complexity: f64,
}

impl Metrics {
    fn new(
        wcc: f64,
        crap: f64,
        skunk: f64,
        is_complex: Option<bool>,
        coverage: f64,
        complexity: f64,
    ) -> Self {
        Self {
            wcc,
            crap,
            skunk,
            is_complex,
            coverage,
            complexity,
        }
    }
}

/// Metrics of the project.
#[derive(Debug, Serialize)]
pub struct ProjectMetrics {
    #[serde(skip_serializing)]
    n_files: f64,
    #[serde(skip_serializing)]
    total_lines: f64,
    #[serde(skip_serializing)]
    covered_lines: f64,
    #[serde(skip_serializing)]
    sloc_sum: f64,
    #[serde(skip_serializing)]
    wcc_sum: f64,
    #[serde(skip_serializing)]
    wcc_percentage_sum: f64,
    #[serde(skip_serializing)]
    crap_sum: f64,
    #[serde(skip_serializing)]
    skunk_sum: f64,
    #[serde(skip_serializing)]
    coverage_sum: f64,
    #[serde(skip_serializing)]
    complexity_sum: f64,
    pub total: Metrics,
    pub min: Metrics,
    pub max: Metrics,
    pub average: Metrics,
}

impl ProjectMetrics {
    fn new() -> Self {
        Self {
            n_files: 0.0,
            total_lines: 0.0,
            covered_lines: 0.0,
            sloc_sum: 0.0,
            wcc_sum: 0.0,
            wcc_percentage_sum: 0.0,
            crap_sum: 0.0,
            skunk_sum: 0.0,
            coverage_sum: 0.0,
            complexity_sum: 0.0,
            total: Metrics::new(0.0, 0.0, 0.0, None, 0.0, 0.0),
            min: Metrics::new(f64::MAX, f64::MAX, f64::MAX, None, f64::MAX, f64::MAX),
            max: Metrics::new(f64::MIN, f64::MIN, f64::MIN, None, f64::MIN, f64::MIN),
            average: Metrics::new(0.0, 0.0, 0.0, None, 0.0, 0.0),
        }
    }

    fn update(
        &mut self,
        total_lines: f64,
        covered_lines: f64,
        sloc: f64,
        wcc: WccFuncSpace,
        crap: f64,
        skunk: f64,
        coverage: f64,
        complexity: f64,
    ) {
        self.n_files += 1.0;
        self.total_lines += total_lines;
        self.covered_lines += covered_lines;
        self.sloc_sum += sloc;
        self.wcc_sum += wcc.value;
        self.wcc_percentage_sum += wcc.percentage;
        self.crap_sum += crap;
        self.skunk_sum += skunk;
        self.coverage_sum += coverage;
        self.complexity_sum += complexity;
        self.update_min(wcc.percentage, crap, skunk, coverage, complexity);
        self.update_max(wcc.percentage, crap, skunk, coverage, complexity);
    }

    fn merge(&mut self, other: ProjectMetrics) {
        self.n_files += other.n_files;
        self.total_lines += other.total_lines;
        self.covered_lines += other.covered_lines;
        self.sloc_sum += other.sloc_sum;
        self.wcc_sum += other.wcc_sum;
        self.wcc_percentage_sum += other.wcc_percentage_sum;
        self.crap_sum += other.crap_sum;
        self.skunk_sum += other.skunk_sum;
        self.coverage_sum += other.coverage_sum;
        self.complexity_sum += other.complexity_sum;
        self.update_min(
            other.min.wcc,
            other.min.crap,
            other.min.skunk,
            other.min.coverage,
            other.min.complexity,
        );
        self.update_max(
            other.max.wcc,
            other.max.crap,
            other.max.skunk,
            other.max.coverage,
            other.max.complexity,
        );
    }

    fn update_min(&mut self, wcc: f64, crap: f64, skunk: f64, coverage: f64, complexity: f64) {
        self.min.wcc = self.min.wcc.min(wcc);
        self.min.crap = self.min.crap.min(crap);
        self.min.skunk = self.min.skunk.min(skunk);
        self.min.coverage = self.min.coverage.min(coverage);
        self.min.complexity = self.min.complexity.min(complexity);
    }

    fn update_max(&mut self, wcc: f64, crap: f64, skunk: f64, coverage: f64, complexity: f64) {
        self.max.wcc = self.max.wcc.max(wcc);
        self.max.crap = self.max.crap.max(crap);
        self.max.skunk = self.max.skunk.max(skunk);
        self.max.coverage = self.max.coverage.max(coverage);
        self.max.complexity = self.max.complexity.max(complexity);
    }

    fn commpute_total(&mut self) {
        let project_coverage = self.covered_lines / self.total_lines;
        self.total.wcc = round_sd((self.wcc_sum / self.sloc_sum) * 100.0);
        self.total.crap = round_sd(crap(project_coverage, self.complexity_sum));
        self.total.skunk = round_sd(skunk(project_coverage, self.complexity_sum));
        self.total.coverage = round_sd(project_coverage * 100.0);
        self.total.complexity = round_sd(self.complexity_sum);
    }

    fn compute_average(&mut self) {
        self.average.wcc = round_sd(self.wcc_percentage_sum / self.n_files);
        self.average.crap = round_sd(self.crap_sum / self.n_files);
        self.average.skunk = round_sd(self.skunk_sum / self.n_files);
        self.average.coverage = round_sd(self.coverage_sum / self.n_files);
        self.average.complexity = round_sd(self.complexity_sum / self.n_files);
    }
}

/// Output of the weighted code coverage.
#[derive(Debug, Serialize)]
pub struct WccOutput {
    pub files: Vec<FileMetrics>,
    pub project: ProjectMetrics,
    pub ignored_files: Vec<String>,
}

pub(crate) struct Wcc<P: AsRef<Path>> {
    project_path: P,
    files: Vec<PathBuf>,
    mode: Mode,
    grcov: Grcov,
    metric: Complexity,
    thresholds: Vec<f64>,
    files_metrics: Mutex<Vec<FileMetrics>>,
    ignored_files: Mutex<Vec<String>>,
    sort_by: Sort,
}

impl<'a, P: AsRef<Path>> Wcc<P> {
    pub(crate) fn new(
        project_path: P,
        files: Vec<PathBuf>,
        mode: Mode,
        grcov: Grcov,
        metric: Complexity,
        thresholds: Vec<f64>,
        sort_by: Sort,
    ) -> Self {
        Self {
            project_path,
            files,
            mode,
            grcov,
            metric,
            thresholds,
            ignored_files: Mutex::new(Vec::new()),
            files_metrics: Mutex::new(Vec::new()),
            sort_by,
        }
    }

    fn get_space_coverage(&self, space: &FuncSpace, lines_coverage: &[Option<i32>]) -> f64 {
        let covered_lines = LinesMetrics::get_covered_lines(space, lines_coverage);

        round_sd(covered_lines / space.metrics.loc.sloc())
    }

    fn get_functions<'b>(&'b self, root: &'b FuncSpace) -> Vec<&'b FuncSpace> {
        let mut functions = Vec::new();
        let mut stack = vec![root];

        while let Some(func_space) = stack.pop() {
            if func_space.kind == SpaceKind::Function {
                functions.push(func_space);
            }
            func_space.spaces.iter().for_each(|space| stack.push(space));
        }

        functions
    }

    fn get_complexity(&self, func_space: &FuncSpace) -> f64 {
        match self.metric {
            Complexity::Cyclomatic => func_space.metrics.cyclomatic.cyclomatic_sum(),
            Complexity::Cognitive => func_space.metrics.cognitive.cognitive_sum(),
        }
    }

    fn check_complexity(&self, wcc: f64, crap: f64, skunk: f64) -> bool {
        wcc < self.thresholds[0] || crap > self.thresholds[1] || skunk > self.thresholds[2]
    }

    fn function_name(&self, function: &FuncSpace) -> Result<String> {
        let name = format!(
            "{} ({}, {})",
            function.name.as_ref().ok_or(Error::OptionRefConversion)?,
            function.start_line,
            function.end_line
        );

        Ok(name)
    }

    fn compute_functions_metrics(
        &self,
        functions: &[&FuncSpace],
        lines_coverage: &[Option<i32>],
        wcc_sum: &mut f64,
        sloc_sum: &mut f64,
        functions_metrics: &mut Vec<FunctionMetrics>,
    ) -> Result<()> {
        for function in functions {
            let complexity = self.get_complexity(function);
            let wcc = wcc_func_space(function, lines_coverage, complexity);

            if let Mode::Functions = self.mode {
                let coverage = self.get_space_coverage(function, lines_coverage);
                let coverage_percentage = round_sd(coverage * 100.0);
                let crap = crap(coverage, complexity);
                let skunk = skunk(coverage, complexity);
                let is_complex = self.check_complexity(wcc.percentage, crap, skunk);
                let metrics = Metrics::new(
                    wcc.percentage,
                    crap,
                    skunk,
                    Some(is_complex),
                    coverage_percentage,
                    complexity,
                );
                let function_name = self.function_name(function)?;
                functions_metrics.push(FunctionMetrics::new(function_name, metrics));
            }

            *wcc_sum += wcc.value;
            *sloc_sum += function.metrics.loc.sloc();
        }

        Ok(())
    }

    fn compute_file_metrics<F: AsRef<Path>>(
        &self,
        file: F,
        project_metrics: &mut ProjectMetrics,
    ) -> Result<()> {
        let lines_coverage = match self.grcov.get_lines_coverage(&file) {
            Some(c) => c,
            None => {
                let mut ignored_files = self.ignored_files.lock()?;
                ignored_files.push(
                    file.as_ref()
                        .to_path_buf()
                        .strip_prefix(self.project_path.as_ref())?
                        .to_str()
                        .ok_or(Error::Conversion)?
                        .to_string(),
                );

                return Ok(());
            }
        };

        let root = get_root(&file)?;
        let functions = self.get_functions(&root);
        let mut wcc_sum = 0.0;
        let mut sloc_sum = 0.0;
        let mut functions_metrics = Vec::new();
        self.compute_functions_metrics(
            &functions,
            lines_coverage,
            &mut wcc_sum,
            &mut sloc_sum,
            &mut functions_metrics,
        )?;

        let lines_metrics = LinesMetrics::new(&root, lines_coverage);
        let complexity = self.get_complexity(&root);
        let coverage = self.get_space_coverage(&root, lines_coverage);
        let coverage_percentage = round_sd(coverage * 100.0);
        let wcc = wcc_file(wcc_sum, sloc_sum);
        let crap = crap(coverage, complexity);
        let skunk = skunk(coverage, complexity);
        let is_complex = self.check_complexity(wcc.percentage, crap, skunk);
        let file_name = self.grcov.get_file_name(file, &self.project_path)?;
        let metrics = Metrics::new(
            wcc.percentage,
            crap,
            skunk,
            Some(is_complex),
            coverage_percentage,
            complexity,
        );
        let functions_metrics = match self.mode {
            Mode::Files => None,
            Mode::Functions => Some(functions_metrics),
        };

        project_metrics.update(
            lines_metrics.total_lines,
            lines_metrics.covered_lines,
            sloc_sum,
            wcc,
            crap,
            skunk,
            coverage_percentage,
            complexity,
        );

        let mut files_metrics = self.files_metrics.lock()?;
        files_metrics.push(FileMetrics::new(file_name, metrics, functions_metrics));

        Ok(())
    }
}

impl<'a, P: AsRef<Path>> WccConcurrent for Wcc<P> {
    type ProducerItem = PathBuf;
    type ConsumerItem = ProjectMetrics;
    type Output = WccOutput;

    fn producer(&self, sender: Sender<Self::ProducerItem>) -> Result<()> {
        for f in &self.files {
            sender.send(f.to_owned())?;
        }

        Ok(())
    }

    fn consumer(
        &self,
        receiver: Receiver<Self::ProducerItem>,
        sender: Sender<Self::ConsumerItem>,
    ) -> Result<()> {
        let mut project_metrics = ProjectMetrics::new();
        while let Ok(file) = receiver.recv() {
            self.compute_file_metrics(file, &mut project_metrics)?;
        }

        sender.send(project_metrics)?;

        Ok(())
    }

    fn composer(&self, receiver: Receiver<Self::ConsumerItem>) -> Result<Self::Output> {
        let mut project_metrics = ProjectMetrics::new();
        while let Ok(consumer_project_metrics) = receiver.recv() {
            project_metrics.merge(consumer_project_metrics);
        }
        project_metrics.compute_average();
        project_metrics.commpute_total();

        let metrics = self.files_metrics.lock()?;
        let ignored_files = self.ignored_files.lock()?;

        Ok(WccOutput {
            files: metrics.clone(),
            project: project_metrics,
            ignored_files: ignored_files.clone(),
        })
    }
}
