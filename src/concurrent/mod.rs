pub(crate) mod files;
pub(crate) mod functions;

use std::{
    collections::HashMap,
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
        crap::crap,
        get_line_space, get_root, get_space_name, round_sd,
        skunk::skunk,
        wcc::{wcc, wcc_function, WCC_COMPLEXITY_THRESHOLD},
        MetricsThresholds,
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
    fn run(self, n_threads: usize) -> Result<Self::Output>
    where
        Self: Sync + Sized,
    {
        let (producer_sender, consumer_receiver) = crossbeam::channel::bounded(n_threads);
        let (consumer_sender, composer_receiver) = crossbeam::channel::bounded(n_threads);

        crossbeam::thread::scope(|scope| {
            // Producer
            scope.spawn(|_| self.producer(producer_sender));

            // Composer
            let composer = scope.spawn(|_| self.composer(composer_receiver));

            // Consumer.
            (0..n_threads).into_par_iter().try_for_each(|_| {
                self.consumer(consumer_receiver.clone(), consumer_sender.clone())
            })?;

            // The Sender between consumers and composer must be dropped so that shared channels can be closed.
            // Otherwise, the composer will eternally await data from the consumers.
            drop(consumer_sender);

            // Result produced by the composer.
            composer.join()?
        })
        .map_err(Into::<Error>::into)?
    }
}

pub(crate) enum Grcov {
    Coveralls(Coveralls),
    Covdir(Covdir),
}

impl Grcov {
    #[inline]
    fn get_lines_coverage(&self, file: &Path) -> Option<&Vec<Option<i32>>> {
        match self {
            Grcov::Coveralls(coveralls) => coveralls.0.get(file).map(|c| &c.coverage),
            Grcov::Covdir(covdir) => covdir.source_files.get(file).map(|c| &c.coverage),
        }
    }

    fn get_file_name<'a>(&'a self, file: &'a Path, project_path: &Path) -> Option<&str> {
        match self {
            Grcov::Coveralls(coveralls) => coveralls.0.get(file)?.name.to_str(),
            Grcov::Covdir(_) => file.strip_prefix(project_path).ok()?.to_str(),
        }
    }
}

/// Metrics data.
#[derive(Debug, Serialize, Clone, Copy, Default)]
#[serde(rename_all = "camelCase")]
pub struct MetricsData {
    /// Wcc.
    pub wcc: f64,
    /// CRAP.
    pub crap: f64,
    /// Skunk.
    pub skunk: f64,
    /// Complexity.
    pub complexity: f64,
    /// Inidcates whether one of the metrics exceeds the threshold.
    pub is_complex: bool,
}

impl MetricsData {
    fn file(
        project_data: ProjectData,
        metrics_thresholds: MetricsThresholds,
        complexity_type: Complexity,
    ) -> Self {
        let coverage = project_data.covered_lines / project_data.ploc;
        let (complexity, wcc_coverage) = match complexity_type {
            Complexity::Cyclomatic => (
                project_data.cyclomatic_complexity / project_data.num_spaces,
                project_data.wcc_cyclomatic_coverage,
            ),
            Complexity::Cognitive => (
                project_data.cognitive_complexity / project_data.num_spaces,
                project_data.wcc_cognitive_coverage,
            ),
        };

        let wcc = wcc(wcc_coverage, project_data.ploc);
        let crap = crap(coverage, complexity);
        let skunk = skunk(coverage, complexity);

        Self {
            wcc,
            crap,
            skunk,
            complexity: round_sd(complexity),
            is_complex: metrics_thresholds.is_complex(wcc, crap, skunk, complexity_type),
        }
    }

    fn function(
        space_data: SpaceData,
        metrics_thresholds: MetricsThresholds,
        complexity_type: Complexity,
    ) -> Self {
        let coverage = space_data.covered_lines / space_data.ploc;
        let complexity = match complexity_type {
            Complexity::Cyclomatic => space_data.cyclomatic_complexity,
            Complexity::Cognitive => space_data.cognitive_complexity,
        };

        let wcc = wcc_function(complexity, space_data.covered_lines, space_data.ploc);
        let crap = crap(coverage, complexity);
        let skunk = skunk(coverage, complexity);

        Self {
            wcc,
            crap,
            skunk,
            complexity,
            is_complex: metrics_thresholds.is_complex(wcc, crap, skunk, complexity_type),
        }
    }

    fn project_total(
        project_data: ProjectData,
        metrics_thresholds: MetricsThresholds,
        complexity_type: Complexity,
    ) -> Self {
        let coverage = project_data.covered_lines / project_data.ploc;
        let (complexity, wcc_coverage) = match complexity_type {
            Complexity::Cyclomatic => (
                project_data.cyclomatic_complexity / project_data.num_spaces,
                project_data.wcc_cyclomatic_coverage,
            ),
            Complexity::Cognitive => (
                project_data.cognitive_complexity / project_data.num_spaces,
                project_data.wcc_cognitive_coverage,
            ),
        };

        let wcc = round_sd((wcc_coverage / project_data.ploc) * 100.0);
        let crap = crap(coverage, complexity);
        let skunk = skunk(coverage, complexity);

        Self {
            wcc,
            crap,
            skunk,
            complexity,
            is_complex: metrics_thresholds.is_complex(wcc, crap, skunk, complexity_type),
        }
    }

    const fn project_min() -> Self {
        Self {
            wcc: f64::MAX,
            crap: f64::MAX,
            skunk: f64::MAX,
            complexity: f64::MAX,
            is_complex: false,
        }
    }

    #[inline]
    fn update_project_min(
        mut self,
        other: MetricsData,
        metrics_thresholds: MetricsThresholds,
        complexity: Complexity,
    ) -> Self {
        self.wcc = self.wcc.min(other.wcc);
        self.crap = self.crap.min(other.crap);
        self.skunk = self.skunk.min(other.skunk);
        self.complexity = self.complexity.min(other.complexity);
        self.is_complex =
            metrics_thresholds.is_complex(self.wcc, self.crap, self.skunk, complexity);

        self
    }

    const fn project_max() -> Self {
        Self {
            wcc: f64::MIN,
            crap: f64::MIN,
            skunk: f64::MIN,
            complexity: f64::MIN,
            is_complex: false,
        }
    }

    #[inline]
    fn update_project_max(
        mut self,
        other: MetricsData,
        metrics_thresholds: MetricsThresholds,
        complexity: Complexity,
    ) -> Self {
        self.wcc = self.wcc.max(other.wcc);
        self.crap = self.crap.max(other.crap);
        self.skunk = self.skunk.max(other.skunk);
        self.complexity = self.complexity.max(other.complexity);
        self.is_complex =
            metrics_thresholds.is_complex(self.wcc, self.crap, self.skunk, complexity);

        self
    }

    #[inline]
    fn sum(mut self, other: MetricsData) -> Self {
        self.wcc += other.wcc;
        self.crap += other.crap;
        self.skunk += other.skunk;
        self.complexity += other.complexity;

        self
    }

    fn project_average(
        self,
        num_files: f64,
        metrics_thresholds: MetricsThresholds,
        complexity: Complexity,
    ) -> Self {
        let wcc = round_sd(self.wcc / num_files);
        let crap = round_sd(self.crap / num_files);
        let skunk = round_sd(self.skunk / num_files);

        Self {
            wcc,
            crap,
            skunk,
            complexity: round_sd(self.complexity / num_files),
            is_complex: metrics_thresholds.is_complex(wcc, crap, skunk, complexity),
        }
    }
}

/// Metrics.
#[derive(Debug, Serialize, Clone, Copy, Default)]
pub struct Metrics {
    /// Cyclomatic.
    pub cyclomatic: MetricsData,
    /// Cognitive.
    pub cognitive: MetricsData,
    /// Coverage.
    pub coverage: f64,
}

impl Metrics {
    fn file(project_data: ProjectData, metrics_thresholds: MetricsThresholds) -> Self {
        Self {
            cyclomatic: MetricsData::file(project_data, metrics_thresholds, Complexity::Cyclomatic),
            cognitive: MetricsData::file(project_data, metrics_thresholds, Complexity::Cognitive),
            coverage: round_sd((project_data.covered_lines / project_data.ploc) * 100.0),
        }
    }

    fn function(space_data: SpaceData, metrics_thresholds: MetricsThresholds) -> Self {
        Self {
            cyclomatic: MetricsData::function(
                space_data,
                metrics_thresholds,
                Complexity::Cyclomatic,
            ),
            cognitive: MetricsData::function(space_data, metrics_thresholds, Complexity::Cognitive),
            coverage: round_sd((space_data.covered_lines / space_data.ploc) * 100.0),
        }
    }

    fn project_total(project_data: ProjectData, metrics_thresholds: MetricsThresholds) -> Self {
        let coverage = round_sd((project_data.covered_lines / project_data.ploc) * 100.0);
        let cyclomatic =
            MetricsData::project_total(project_data, metrics_thresholds, Complexity::Cyclomatic);
        let cognitive =
            MetricsData::project_total(project_data, metrics_thresholds, Complexity::Cognitive);

        Self {
            cyclomatic,
            cognitive,
            coverage,
        }
    }

    const fn project_min() -> Self {
        Self {
            cyclomatic: MetricsData::project_min(),
            cognitive: MetricsData::project_min(),
            coverage: f64::MAX,
        }
    }

    fn update_project_min(mut self, other: Metrics, metrics_thresholds: MetricsThresholds) -> Self {
        self.cyclomatic = self.cyclomatic.update_project_min(
            other.cyclomatic,
            metrics_thresholds,
            Complexity::Cyclomatic,
        );
        self.cognitive = self.cognitive.update_project_min(
            other.cognitive,
            metrics_thresholds,
            Complexity::Cognitive,
        );
        self.coverage = self.coverage.min(other.coverage);

        self
    }

    const fn project_max() -> Self {
        Self {
            cyclomatic: MetricsData::project_max(),
            cognitive: MetricsData::project_max(),
            coverage: f64::MIN,
        }
    }

    fn update_project_max(mut self, other: Metrics, metrics_thresholds: MetricsThresholds) -> Self {
        self.cyclomatic = self.cyclomatic.update_project_max(
            other.cyclomatic,
            metrics_thresholds,
            Complexity::Cyclomatic,
        );
        self.cognitive = self.cognitive.update_project_max(
            other.cognitive,
            metrics_thresholds,
            Complexity::Cognitive,
        );
        self.coverage = self.coverage.max(other.coverage);

        self
    }

    #[inline]
    fn project_sum(mut self, other: Metrics) -> Self {
        self.cyclomatic = self.cyclomatic.sum(other.cyclomatic);
        self.cognitive = self.cognitive.sum(other.cognitive);
        self.coverage += other.coverage;

        self
    }

    fn project_average(self, num_files: f64, metrics_thresholds: MetricsThresholds) -> Self {
        let cyclomatic =
            self.cyclomatic
                .project_average(num_files, metrics_thresholds, Complexity::Cyclomatic);
        let cognitive =
            self.cognitive
                .project_average(num_files, metrics_thresholds, Complexity::Cognitive);
        let coverage = round_sd(self.coverage / num_files);

        Self {
            cyclomatic,
            cognitive,
            coverage,
        }
    }
}

/// Project metrics.
#[derive(Debug, Serialize)]
pub struct ProjectMetrics {
    /// Total.
    pub total: Metrics,
    /// Minimum.
    pub min: Metrics,
    /// Maximum.
    pub max: Metrics,
    /// Average.
    pub average: Metrics,
}

impl ProjectMetrics {
    const fn new(total: Metrics, min: Metrics, max: Metrics, average: Metrics) -> Self {
        Self {
            total,
            min,
            max,
            average,
        }
    }
}

#[derive(Debug, Default, Clone, Copy)]
pub(crate) struct ProjectData {
    num_spaces: f64,
    ploc: f64,
    covered_lines: f64,
    wcc_cyclomatic_coverage: f64,
    wcc_cognitive_coverage: f64,
    cyclomatic_complexity: f64,
    cognitive_complexity: f64,
}

impl ProjectData {
    #[inline]
    fn new(num_spaces: f64) -> Self {
        Self {
            num_spaces,
            ..Default::default()
        }
    }

    fn update(&mut self, space_data: &SpaceData) {
        self.covered_lines += space_data.covered_lines;
        if space_data.cyclomatic_complexity <= WCC_COMPLEXITY_THRESHOLD {
            self.wcc_cyclomatic_coverage += space_data.covered_lines;
        }
        if space_data.cognitive_complexity <= WCC_COMPLEXITY_THRESHOLD {
            self.wcc_cognitive_coverage += space_data.covered_lines;
        }

        self.ploc += space_data.ploc;
        self.cyclomatic_complexity += space_data.cyclomatic_complexity;
        self.cognitive_complexity += space_data.cognitive_complexity;
    }

    #[inline]
    fn merge(&mut self, other: ProjectData) {
        self.num_spaces += other.num_spaces;
        self.ploc += other.ploc;
        self.covered_lines += other.covered_lines;
        self.wcc_cyclomatic_coverage += other.wcc_cyclomatic_coverage;
        self.wcc_cognitive_coverage += other.wcc_cognitive_coverage;
        self.cyclomatic_complexity += other.cyclomatic_complexity;
        self.cognitive_complexity += other.cognitive_complexity;
    }
}

/// Output of the weighted code coverage.
#[derive(Debug, Serialize)]
pub struct WccOutput {
    /// Files.
    pub files: Vec<FileMetrics>,
    /// Project.
    pub project: ProjectMetrics,
    /// Ignored files.
    pub ignored_files: Vec<String>,
}

impl WccOutput {
    const fn new(
        files: Vec<FileMetrics>,
        project: ProjectMetrics,
        ignored_files: Vec<String>,
    ) -> Self {
        Self {
            files,
            project,
            ignored_files,
        }
    }
}

#[derive(Clone, Copy)]
pub(crate) struct SpaceData {
    ploc: f64,
    covered_lines: f64,
    cyclomatic_complexity: f64,
    cognitive_complexity: f64,
    kind: SpaceKind,
}

pub(crate) struct Wcc<'a> {
    pub(crate) project_path: &'a Path,
    pub(crate) files: &'a [PathBuf],
    pub(crate) mode: Mode,
    pub(crate) grcov: Grcov,
    pub(crate) metrics_thresholds: MetricsThresholds,
    pub(crate) files_metrics: Mutex<Vec<FileMetrics>>,
    pub(crate) ignored_files: Mutex<Vec<String>>,
    pub(crate) sort_by: Sort,
}

impl<'a> Wcc<'a> {
    fn update_ignored_files(&self, file: &Path) -> Result<()> {
        let mut ignored_files = self.ignored_files.lock()?;
        if let Some(file) = file.strip_prefix(self.project_path)?.to_str() {
            ignored_files.push(file.to_string())
        }

        Ok(())
    }

    fn sort_output(&self) -> Result<()> {
        let sort = |a: Metrics, b: Metrics| match self.sort_by {
            Sort::Wcc => b.cyclomatic.wcc.total_cmp(&a.cyclomatic.wcc),
            Sort::Crap => b.cyclomatic.crap.total_cmp(&a.cyclomatic.crap),
            Sort::Skunk => b.cyclomatic.skunk.total_cmp(&a.cyclomatic.skunk),
        };

        let mut files_metrics = self.files_metrics.lock()?;
        files_metrics.sort_by(|a, b| sort(a.metrics, b.metrics));
        files_metrics.iter_mut().for_each(|fm| {
            if let Some(ref mut functions) = fm.functions {
                functions.sort_by(|a, b| sort(a.metrics, b.metrics));
            }
        });

        let mut ignored_files = self.ignored_files.lock()?;
        ignored_files.sort();

        Ok(())
    }

    fn update_spaces(
        &self,
        space: &FuncSpace,
        spaces: &mut HashMap<String, SpaceData>,
        line_is_covered: bool,
    ) {
        if let Some(key) = get_space_name(space) {
            spaces
                .entry(key.to_owned())
                .and_modify(|space_data| {
                    space_data.ploc += 1.0;
                    if line_is_covered {
                        space_data.covered_lines += 1.0;
                    }
                })
                .or_insert(SpaceData {
                    ploc: 1.0,
                    covered_lines: if line_is_covered { 1.0 } else { 0.0 },
                    cyclomatic_complexity: space.metrics.cyclomatic.cyclomatic_sum(),
                    cognitive_complexity: space.metrics.cognitive.cognitive_sum(),
                    kind: space.kind,
                });
        }
    }

    fn get_functions_metrics(
        &self,
        spaces: HashMap<String, SpaceData>,
    ) -> Option<Vec<FunctionMetrics>> {
        if let Mode::Files = self.mode {
            return None;
        }

        let functions: Vec<FunctionMetrics> = spaces
            .into_iter()
            .filter(|(_, data)| data.kind == SpaceKind::Function)
            .map(|(name, space_data)| {
                FunctionMetrics::new(name, space_data, self.metrics_thresholds)
            })
            .collect();

        (!functions.is_empty()).then_some(functions)
    }

    fn compute_file_metrics(
        &self,
        file: &Path,
        spaces: HashMap<String, SpaceData>,
    ) -> Result<ProjectData> {
        let mut project_data = ProjectData::new(spaces.len() as f64);
        spaces
            .values()
            .for_each(|space_data| project_data.update(space_data));

        let mut files_metrics = self.files_metrics.lock()?;
        if let Some(name) = self.grcov.get_file_name(file, self.project_path) {
            files_metrics.push(FileMetrics::new(
                name.to_owned(),
                project_data,
                self.metrics_thresholds,
                self.get_functions_metrics(spaces),
            ));
        }

        Ok(project_data)
    }

    fn get_spaces(
        &self,
        file: &Path,
        lines_coverage: &[Option<i32>],
    ) -> Result<HashMap<String, SpaceData>> {
        let mut spaces: HashMap<String, SpaceData> = HashMap::new();
        let root = get_root(file)?;

        for (line, coverage) in lines_coverage
            .iter()
            .enumerate()
            .filter_map(|(line, coverage)| coverage.map(|cov| (line, cov)))
        {
            let space = get_line_space(&root, line);
            self.update_spaces(space, &mut spaces, coverage != 0);
        }

        Ok(spaces)
    }

    fn compute_metrics(&self, file: &Path) -> Option<ProjectData> {
        let lines_coverage = if let Some(lines_coverage) = self.grcov.get_lines_coverage(file) {
            lines_coverage
        } else {
            self.update_ignored_files(file).ok()?;
            return None;
        };
        let spaces = self.get_spaces(file, lines_coverage).ok()?;

        self.compute_file_metrics(file, spaces).ok()
    }

    fn get_project_min(&self) -> Result<Metrics> {
        let metrics = self.files_metrics.lock()?.iter().fold(
            Metrics::project_min(),
            |min_metrics, file_metrics| {
                min_metrics.update_project_min(file_metrics.metrics, self.metrics_thresholds)
            },
        );

        Ok(metrics)
    }

    fn get_project_max(&self) -> Result<Metrics> {
        let metrics = self.files_metrics.lock()?.iter().fold(
            Metrics::project_max(),
            |max_metrics, file_metrics| {
                max_metrics.update_project_max(file_metrics.metrics, self.metrics_thresholds)
            },
        );

        Ok(metrics)
    }

    fn get_project_average(&self) -> Result<Metrics> {
        let files_metrics = self.files_metrics.lock()?;
        let sum_metrics = files_metrics
            .iter()
            .fold(Metrics::default(), |sum_metrics, file_metrics| {
                sum_metrics.project_sum(file_metrics.metrics)
            });

        Ok(sum_metrics.project_average(files_metrics.len() as f64, self.metrics_thresholds))
    }

    fn get_project_metrics(&self, project_data: ProjectData) -> Result<ProjectMetrics> {
        let total = Metrics::project_total(project_data, self.metrics_thresholds);
        let min = self.get_project_min()?;
        let max = self.get_project_max()?;
        let average = self.get_project_average()?;

        Ok(ProjectMetrics::new(total, min, max, average))
    }
}

impl<'a> WccConcurrent for Wcc<'a> {
    type ProducerItem = &'a Path;
    type ConsumerItem = ProjectData;
    type Output = WccOutput;

    fn producer(&self, sender: Sender<Self::ProducerItem>) -> Result<()> {
        for f in self.files {
            sender.send(f)?;
        }

        Ok(())
    }

    fn consumer(
        &self,
        receiver: Receiver<Self::ProducerItem>,
        sender: Sender<Self::ConsumerItem>,
    ) -> Result<()> {
        let mut project_data = ProjectData::default();
        while let Ok(file) = receiver.recv() {
            if let Some(file_data) = self.compute_metrics(file) {
                project_data.merge(file_data);
            }
        }
        sender.send(project_data)?;

        Ok(())
    }

    fn composer(&self, receiver: Receiver<Self::ConsumerItem>) -> Result<Self::Output> {
        let mut project_data = ProjectData::default();
        while let Ok(consumer_project_data) = receiver.recv() {
            project_data.merge(consumer_project_data);
        }

        let project_metrics = self.get_project_metrics(project_data)?;
        self.sort_output()?;

        Ok(WccOutput::new(
            self.files_metrics.lock()?.clone(),
            project_metrics,
            self.ignored_files.lock()?.clone(),
        ))
    }
}
