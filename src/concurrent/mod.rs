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
    fn get_lines_coverage(&self, file: &Path) -> Option<&[Option<i32>]> {
        match self {
            Grcov::Coveralls(coveralls) => Some(&coveralls.0.get(file)?.coverage),
            Grcov::Covdir(covdir) => Some(&covdir.source_files.get(file)?.coverage),
        }
    }

    fn get_file_name(&self, file: &Path, project_path: &Path) -> Result<String> {
        match self {
            Grcov::Coveralls(coveralls) => Ok(coveralls
                .0
                .get(file)
                .ok_or(Error::HashMap)?
                .name
                .to_str()
                .ok_or(Error::Conversion)?
                .to_string()),
            Grcov::Covdir(_) => Ok(file
                .to_path_buf()
                .strip_prefix(project_path)?
                .to_str()
                .ok_or(Error::Conversion)?
                .to_string()),
        }
    }
}

/// Metrics data.
#[derive(Debug, Serialize, Clone, Copy)]
#[serde(rename_all = "camelCase")]
pub struct MetricsData {
    /// Wcc.
    pub wcc: f64,
    /// CRAP.
    pub crap: f64,
    /// Skunk.
    pub skunk: f64,
    /// Complexity.
    pub complexity: Option<f64>,
    /// Inidcates whether one of the metrics exceeds the threshold.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_complex: Option<bool>,
}

/// Metrics.
#[derive(Debug, Serialize, Clone, Copy)]
pub struct Metrics {
    /// Cyclomatic.
    pub cyclomatic: MetricsData,
    /// Cognitive.
    pub cognitive: MetricsData,
    /// Coverage.
    pub coverage: f64,
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

#[derive(Default)]
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

struct SpaceData {
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
    pub(crate) thresholds: MetricsThresholds,
    pub(crate) files_metrics: Mutex<Vec<FileMetrics>>,
    pub(crate) ignored_files: Mutex<Vec<String>>,
    pub(crate) sort_by: Sort,
}

impl<'a> Wcc<'a> {
    fn update_ignored_files(&self, file: &Path) -> Result<()> {
        let mut ignored_files = self.ignored_files.lock()?;
        ignored_files.push(
            file.to_path_buf()
                .strip_prefix(self.project_path)?
                .to_str()
                .ok_or(Error::Conversion)?
                .to_string(),
        );

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
    ) -> Result<()> {
        let key = get_space_name(space)?;
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

        Ok(())
    }

    fn get_functions_metrics(
        &self,
        spaces: HashMap<String, SpaceData>,
    ) -> Option<Vec<FunctionMetrics>> {
        let functions: Vec<FunctionMetrics> = spaces
            .into_iter()
            .filter(|(_, data)| data.kind == SpaceKind::Function)
            .map(|(s, data)| {
                let coverage = data.covered_lines / data.ploc;
                let wcc_cyclomatic =
                    wcc_function(data.cyclomatic_complexity, data.covered_lines, data.ploc);
                let wcc_cognitive =
                    wcc_function(data.cognitive_complexity, data.covered_lines, data.ploc);
                let crap_cyclomatic = crap(coverage, data.cyclomatic_complexity);
                let crap_cognitive = crap(coverage, data.cognitive_complexity);
                let skunk_cyclomatic = skunk(coverage, data.cyclomatic_complexity);
                let skunk_cognitive = skunk(coverage, data.cognitive_complexity);

                FunctionMetrics {
                    name: s,
                    metrics: Metrics {
                        cyclomatic: MetricsData {
                            wcc: wcc_cyclomatic,
                            crap: crap_cyclomatic,
                            skunk: skunk_cyclomatic,
                            complexity: Some(data.cyclomatic_complexity),
                            is_complex: Some(self.thresholds.is_complex(
                                wcc_cyclomatic,
                                crap_cyclomatic,
                                skunk_cyclomatic,
                                Complexity::Cyclomatic,
                            )),
                        },
                        cognitive: MetricsData {
                            wcc: wcc_cognitive,
                            crap: crap_cognitive,
                            skunk: skunk_cognitive,
                            complexity: Some(data.cognitive_complexity),
                            is_complex: Some(self.thresholds.is_complex(
                                wcc_cognitive,
                                crap_cognitive,
                                skunk_cognitive,
                                Complexity::Cognitive,
                            )),
                        },
                        coverage: round_sd(coverage * 100.0),
                    },
                }
            })
            .collect();

        match functions.len() {
            0 => None,
            _ => Some(functions),
        }
    }

    fn compute_file_metrics(
        &self,
        file: &Path,
        spaces: HashMap<String, SpaceData>,
    ) -> Result<ProjectData> {
        let mut ploc = 0.0;
        let mut covered_lines = 0.0;
        let mut wcc_cyclomatic_coverage = 0.0;
        let mut wcc_cognitive_coverage = 0.0;
        let mut cyclomatic_complexity = 0.0;
        let mut cognitive_complexity = 0.0;
        spaces.values().for_each(|s| {
            covered_lines += s.covered_lines;
            if s.cyclomatic_complexity <= WCC_COMPLEXITY_THRESHOLD {
                wcc_cyclomatic_coverage += s.covered_lines;
            }
            if s.cognitive_complexity <= WCC_COMPLEXITY_THRESHOLD {
                wcc_cognitive_coverage += s.covered_lines;
            }

            ploc += s.ploc;
            cyclomatic_complexity += s.cyclomatic_complexity;
            cognitive_complexity += s.cognitive_complexity;
        });

        let num_spaces = spaces.len() as f64;
        let coverage = covered_lines / ploc;
        let avg_cycl_comp = cyclomatic_complexity / num_spaces;
        let avg_cogn_comp = cognitive_complexity / num_spaces;

        let wcc_cyclomatic = wcc(wcc_cyclomatic_coverage, ploc);
        let crap_cyclomatic = crap(coverage, avg_cycl_comp);
        let skunk_cyclomatic = skunk(coverage, avg_cycl_comp);
        let wcc_cognitive = wcc(wcc_cognitive_coverage, ploc);
        let crap_cognitive = crap(coverage, avg_cogn_comp);
        let skunk_cognitive = skunk(coverage, avg_cogn_comp);

        let mut files_metrics = self.files_metrics.lock()?;
        files_metrics.push(FileMetrics {
            name: self.grcov.get_file_name(file, self.project_path)?,
            metrics: Metrics {
                cyclomatic: MetricsData {
                    wcc: wcc_cyclomatic,
                    crap: crap_cyclomatic,
                    skunk: skunk_cyclomatic,
                    complexity: Some(round_sd(avg_cycl_comp)),
                    is_complex: Some(self.thresholds.is_complex(
                        wcc_cyclomatic,
                        crap_cyclomatic,
                        skunk_cyclomatic,
                        Complexity::Cyclomatic,
                    )),
                },
                cognitive: MetricsData {
                    wcc: wcc_cognitive,
                    crap: crap_cognitive,
                    skunk: skunk_cognitive,
                    complexity: Some(round_sd(avg_cogn_comp)),
                    is_complex: Some(self.thresholds.is_complex(
                        wcc_cognitive,
                        crap_cognitive,
                        skunk_cognitive,
                        Complexity::Cyclomatic,
                    )),
                },
                coverage: round_sd(coverage * 100.0),
            },
            functions: match self.mode {
                Mode::Files => None,
                Mode::Functions => self.get_functions_metrics(spaces),
            },
        });

        Ok(ProjectData {
            num_spaces,
            ploc,
            covered_lines,
            wcc_cyclomatic_coverage,
            wcc_cognitive_coverage,
            cyclomatic_complexity,
            cognitive_complexity,
        })
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
            match coverage {
                0 => self.update_spaces(space, &mut spaces, false)?,
                _ => self.update_spaces(space, &mut spaces, true)?,
            }
        }

        Ok(spaces)
    }

    fn compute_metrics(&self, file: &Path) -> Option<ProjectData> {
        let lines_coverage = match self.grcov.get_lines_coverage(file) {
            Some(c) => c,
            None => {
                self.update_ignored_files(file).ok()?;
                return None;
            }
        };
        let spaces = self.get_spaces(file, lines_coverage).ok()?;

        self.compute_file_metrics(file, spaces).ok()
    }

    fn get_project_min(&self) -> Result<Metrics> {
        let metrics = self.files_metrics.lock()?.iter().fold(
            Metrics {
                cyclomatic: MetricsData {
                    wcc: f64::MAX,
                    crap: f64::MAX,
                    skunk: f64::MAX,
                    complexity: None,
                    is_complex: None,
                },
                cognitive: MetricsData {
                    wcc: f64::MAX,
                    crap: f64::MAX,
                    skunk: f64::MAX,
                    complexity: None,
                    is_complex: None,
                },
                coverage: f64::MAX,
            },
            |min_metrics, file_metrics| Metrics {
                cyclomatic: MetricsData {
                    wcc: min_metrics
                        .cyclomatic
                        .wcc
                        .min(file_metrics.metrics.cyclomatic.wcc),
                    crap: min_metrics
                        .cyclomatic
                        .crap
                        .min(file_metrics.metrics.cyclomatic.crap),
                    skunk: min_metrics
                        .cyclomatic
                        .skunk
                        .min(file_metrics.metrics.cyclomatic.skunk),
                    complexity: None,
                    is_complex: None,
                },
                cognitive: MetricsData {
                    wcc: min_metrics
                        .cognitive
                        .wcc
                        .min(file_metrics.metrics.cognitive.wcc),
                    crap: min_metrics
                        .cognitive
                        .crap
                        .min(file_metrics.metrics.cognitive.crap),
                    skunk: min_metrics
                        .cognitive
                        .skunk
                        .min(file_metrics.metrics.cognitive.skunk),
                    complexity: None,
                    is_complex: None,
                },
                coverage: min_metrics.coverage.min(file_metrics.metrics.coverage),
            },
        );

        Ok(metrics)
    }

    fn get_project_max(&self) -> Result<Metrics> {
        let metrics = self.files_metrics.lock()?.iter().fold(
            Metrics {
                cyclomatic: MetricsData {
                    wcc: f64::MIN,
                    crap: f64::MIN,
                    skunk: f64::MIN,
                    complexity: None,
                    is_complex: None,
                },
                cognitive: MetricsData {
                    wcc: f64::MIN,
                    crap: f64::MIN,
                    skunk: f64::MIN,
                    complexity: None,
                    is_complex: None,
                },
                coverage: f64::MIN,
            },
            |max_metrics, file_metrics| Metrics {
                cyclomatic: MetricsData {
                    wcc: max_metrics
                        .cyclomatic
                        .wcc
                        .max(file_metrics.metrics.cyclomatic.wcc),
                    crap: max_metrics
                        .cyclomatic
                        .crap
                        .max(file_metrics.metrics.cyclomatic.crap),
                    skunk: max_metrics
                        .cyclomatic
                        .skunk
                        .max(file_metrics.metrics.cyclomatic.skunk),
                    complexity: None,
                    is_complex: None,
                },
                cognitive: MetricsData {
                    wcc: max_metrics
                        .cognitive
                        .wcc
                        .max(file_metrics.metrics.cognitive.wcc),
                    crap: max_metrics
                        .cognitive
                        .crap
                        .max(file_metrics.metrics.cognitive.crap),
                    skunk: max_metrics
                        .cognitive
                        .skunk
                        .max(file_metrics.metrics.cognitive.skunk),
                    complexity: None,
                    is_complex: None,
                },
                coverage: max_metrics.coverage.max(file_metrics.metrics.coverage),
            },
        );

        Ok(metrics)
    }

    fn get_project_average(&self) -> Result<Metrics> {
        let files_metrics = self.files_metrics.lock()?;
        let num_files = files_metrics.len() as f64;
        let sum_metrics = files_metrics.iter().fold(
            Metrics {
                cyclomatic: MetricsData {
                    wcc: 0.0,
                    crap: 0.0,
                    skunk: 0.0,
                    complexity: None,
                    is_complex: None,
                },
                cognitive: MetricsData {
                    wcc: 0.0,
                    crap: 0.0,
                    skunk: 0.0,
                    complexity: None,
                    is_complex: None,
                },
                coverage: 0.0,
            },
            |sum_metrics, file_metrics| Metrics {
                cyclomatic: MetricsData {
                    wcc: sum_metrics.cyclomatic.wcc + file_metrics.metrics.cyclomatic.wcc,
                    crap: sum_metrics.cyclomatic.crap + file_metrics.metrics.cyclomatic.crap,
                    skunk: sum_metrics.cyclomatic.skunk + file_metrics.metrics.cyclomatic.skunk,
                    complexity: None,
                    is_complex: None,
                },
                cognitive: MetricsData {
                    wcc: sum_metrics.cognitive.wcc + file_metrics.metrics.cognitive.wcc,
                    crap: sum_metrics.cognitive.crap + file_metrics.metrics.cognitive.crap,
                    skunk: sum_metrics.cognitive.skunk + file_metrics.metrics.cognitive.skunk,
                    complexity: None,
                    is_complex: None,
                },
                coverage: sum_metrics.coverage + file_metrics.metrics.coverage,
            },
        );

        Ok(Metrics {
            cyclomatic: MetricsData {
                wcc: round_sd(sum_metrics.cyclomatic.wcc / num_files),
                crap: round_sd(sum_metrics.cyclomatic.crap / num_files),
                skunk: round_sd(sum_metrics.cyclomatic.skunk / num_files),
                complexity: None,
                is_complex: None,
            },
            cognitive: MetricsData {
                wcc: round_sd(sum_metrics.cognitive.wcc / num_files),
                crap: round_sd(sum_metrics.cognitive.crap / num_files),
                skunk: round_sd(sum_metrics.cognitive.skunk / num_files),
                complexity: None,
                is_complex: None,
            },
            coverage: round_sd(sum_metrics.coverage / num_files),
        })
    }

    fn get_project_total(&self, project_data: ProjectData) -> Metrics {
        let coverage = project_data.covered_lines / project_data.ploc;
        let avg_cycl_comp = project_data.cyclomatic_complexity / project_data.num_spaces;
        let avg_cogn_comp = project_data.cognitive_complexity / project_data.num_spaces;

        Metrics {
            cyclomatic: MetricsData {
                wcc: round_sd((project_data.wcc_cyclomatic_coverage / project_data.ploc) * 100.0),
                crap: crap(coverage, avg_cycl_comp),
                skunk: skunk(coverage, avg_cycl_comp),
                complexity: None,
                is_complex: None,
            },
            cognitive: MetricsData {
                wcc: round_sd((project_data.wcc_cognitive_coverage / project_data.ploc) * 100.0),
                crap: crap(coverage, avg_cogn_comp),
                skunk: skunk(coverage, avg_cogn_comp),
                complexity: None,
                is_complex: None,
            },
            coverage: round_sd(coverage * 100.0),
        }
    }

    fn get_project_metrics(&self, project_data: ProjectData) -> Result<ProjectMetrics> {
        let min = self.get_project_min()?;
        let max = self.get_project_max()?;
        let average = self.get_project_average()?;
        let total = self.get_project_total(project_data);

        Ok(ProjectMetrics {
            min,
            max,
            average,
            total,
        })
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

        Ok(WccOutput {
            files: self.files_metrics.lock()?.clone(),
            project: project_metrics,
            ignored_files: self.ignored_files.lock()?.clone(),
        })
    }
}
