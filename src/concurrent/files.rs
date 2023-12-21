use std::path::Path;
use std::sync::Mutex;

use crossbeam::channel::{Receiver, Sender};
use serde::{Deserialize, Serialize};
use tracing::debug;

use crate::concurrent::Visit;
use crate::grcov::covdir::Covdir;
use crate::grcov::coveralls::Coveralls;
use crate::metrics::{get_covered_lines, get_root};
use crate::{error::*, Complexity, Sort};

use super::{get_cumulative_values, get_project_metrics, ConsumerOutputWcc, Metrics, Tree, Wcc};

// Struct with all the metrics computed for a single file
#[derive(Clone, Default, Debug, Serialize, Deserialize, PartialEq)]
pub(crate) struct FileMetrics {
    pub(crate) metrics: Metrics,
    pub(crate) file: String,
    pub(crate) file_path: String,
}

impl FileMetrics {
    fn new(metrics: Metrics, file: String, file_path: String) -> Self {
        Self {
            metrics,
            file,
            file_path,
        }
    }

    fn avg(m: Metrics) -> Self {
        Self {
            metrics: m,
            file: "AVG".into(),
            file_path: "-".into(),
        }
    }

    fn min(m: Metrics) -> Self {
        Self {
            metrics: m,
            file: "MIN".into(),
            file_path: "-".into(),
        }
    }

    fn max(m: Metrics) -> Self {
        Self {
            metrics: m,
            file: "MAX".into(),
            file_path: "-".into(),
        }
    }
}

pub(crate) struct CoverallsFilesWcc {
    pub(crate) chunks: Vec<Vec<String>>,
    pub(crate) coveralls: Coveralls,
    pub(crate) metric: Complexity,
    pub(crate) prefix: usize,
    pub(crate) thresholds: Vec<f64>,
    pub(crate) files_ignored: Mutex<Vec<String>>,
    pub(crate) files_metrics: Mutex<Vec<FileMetrics>>,
    pub(crate) sort_by: Sort,
}

impl CoverallsFilesWcc {
    pub(crate) fn new(
        chunks: Vec<Vec<String>>,
        coveralls: Coveralls,
        metric: Complexity,
        prefix: usize,
        thresholds: Vec<f64>,
        sort_by: Sort,
    ) -> Self {
        Self {
            chunks,
            coveralls,
            metric,
            prefix,
            thresholds,
            files_ignored: Mutex::new(Vec::<String>::new()),
            files_metrics: Mutex::new(Vec::<FileMetrics>::new()),
            sort_by,
        }
    }
}

impl Wcc for CoverallsFilesWcc {
    type ProducerItem = Vec<String>;
    type ConsumerItem = ConsumerOutputWcc;
    type Output = (Vec<FileMetrics>, Vec<String>, Vec<FileMetrics>, f64);

    fn producer(&self, sender: Sender<Self::ProducerItem>) -> Result<()> {
        for chunk in &self.chunks {
            sender.send(chunk.iter().map(|s| s.to_owned()).collect::<Vec<String>>())?;
        }

        Ok(())
    }

    fn consumer(
        &self,
        receiver: Receiver<Self::ProducerItem>,
        sender: Sender<Self::ConsumerItem>,
    ) -> Result<()> {
        let mut consumer_output = ConsumerOutputWcc::default();

        while let Ok(chunk) = receiver.recv() {
            for file in chunk {
                let path = Path::new(&file);
                let file_name: String = path
                    .file_name()
                    .ok_or(Error::PathConversion)?
                    .to_str()
                    .ok_or(Error::PathConversion)?
                    .into();

                // Get the coverage vector from the coveralls file
                // if not present the file will be added to the files ignored
                let coverage = match self.coveralls.0.get(&file) {
                    Some(source_file) => source_file.coverage.to_vec(),
                    None => {
                        let mut files_ignored = self.files_ignored.lock()?;
                        files_ignored.push(file);
                        continue;
                    }
                };

                let root = get_root(path)?;
                let (covered_lines, tot_lines) =
                    get_covered_lines(&coverage, root.start_line, root.end_line)?;

                debug!(
                    "File: {:?} covered lines: {}  total lines: {}",
                    file, covered_lines, tot_lines
                );

                let ploc = root.metrics.loc.ploc();
                let comp = match self.metric {
                    Complexity::Cyclomatic => root.metrics.cyclomatic.cyclomatic_sum(),
                    Complexity::Cognitive => root.metrics.cognitive.cognitive_sum(),
                };
                let file_path = file.clone().split_off(self.prefix);

                // Upgrade all the global variables and add metrics to the result and complex_files
                let (m, (sp_sum, sq_sum)): (Metrics, (f64, f64)) = Tree::get_metrics_from_space(
                    &root,
                    &coverage,
                    self.metric,
                    None,
                    &self.thresholds,
                )?;
                let mut files_metrics = self.files_metrics.lock()?;

                // Update all shared variables
                consumer_output.covered_lines += covered_lines;
                consumer_output.total_lines += tot_lines;
                consumer_output.ploc_sum += ploc;
                consumer_output.wcc_plain_sum += sp_sum;
                consumer_output.wcc_quantized_sum += sq_sum;
                consumer_output.comp_sum += comp;
                files_metrics.push(FileMetrics::new(m, file_name, file_path));
            }
        }

        sender.send(consumer_output)?;

        Ok(())
    }

    fn composer(&self, receiver: Receiver<Self::ConsumerItem>) -> Result<Self::Output> {
        let mut consumers_total_output = ConsumerOutputWcc::default();
        while let Ok(consumer_output) = receiver.recv() {
            consumers_total_output.update(consumer_output);
        }

        let mut files_ignored = self.files_ignored.lock()?;
        let mut files_metrics = self.files_metrics.lock()?;

        // Get final  metrics for all the project
        let project_metric = FileMetrics::new(
            get_project_metrics(consumers_total_output, None)?,
            "PROJECT".into(),
            "-".into(),
        );

        let project_coverage = project_metric.metrics.coverage;
        files_ignored.sort();
        files_metrics.sort_by(|a, b| a.file.cmp(&b.file));

        // Get AVG MIN MAX and complex files
        let mut complex_files = files_metrics
            .iter()
            .filter(|m| m.metrics.is_complex)
            .cloned()
            .collect::<Vec<FileMetrics>>();
        complex_files.sort_by(|a, b| match self.sort_by {
            Sort::WccPlain => b.metrics.wcc_plain.total_cmp(&a.metrics.wcc_plain),
            Sort::WccQuantized => b.metrics.wcc_quantized.total_cmp(&a.metrics.wcc_quantized),
            Sort::Crap => b.metrics.crap.total_cmp(&a.metrics.crap),
            Sort::Skunk => b.metrics.skunk.total_cmp(&a.metrics.skunk),
        });

        let m = files_metrics
            .iter()
            .map(|metric| metric.metrics)
            .collect::<Vec<Metrics>>();

        let (avg, max, min) = get_cumulative_values(&m);
        files_metrics.push(project_metric);
        files_metrics.push(FileMetrics::avg(avg));
        files_metrics.push(FileMetrics::max(max));
        files_metrics.push(FileMetrics::min(min));

        Ok((
            (*files_metrics).clone(),
            (*files_ignored).clone(),
            complex_files,
            f64::round(project_coverage * 100.) / 100.,
        ))
    }
}

pub(crate) struct CovdirFilesWcc {
    pub(crate) chunks: Vec<Vec<String>>,
    pub(crate) covdir: Covdir,
    pub(crate) metric: Complexity,
    pub(crate) prefix: usize,
    pub(crate) thresholds: Vec<f64>,
    pub(crate) files_ignored: Mutex<Vec<String>>,
    pub(crate) files_metrics: Mutex<Vec<FileMetrics>>,
    pub(crate) sort_by: Sort,
}

impl CovdirFilesWcc {
    pub(crate) fn new(
        chunks: Vec<Vec<String>>,
        covdir: Covdir,
        metric: Complexity,
        prefix: usize,
        thresholds: Vec<f64>,
        sort_by: Sort,
    ) -> Self {
        Self {
            chunks,
            covdir,
            metric,
            prefix,
            thresholds,
            files_ignored: Mutex::new(Vec::<String>::new()),
            files_metrics: Mutex::new(Vec::<FileMetrics>::new()),
            sort_by,
        }
    }
}

impl Wcc for CovdirFilesWcc {
    type ProducerItem = Vec<String>;
    type ConsumerItem = ConsumerOutputWcc;
    type Output = (Vec<FileMetrics>, Vec<String>, Vec<FileMetrics>, f64);

    fn producer(&self, sender: Sender<Self::ProducerItem>) -> Result<()> {
        for chunk in &self.chunks {
            sender.send(chunk.iter().map(|s| s.to_owned()).collect::<Vec<String>>())?;
        }

        Ok(())
    }

    fn consumer(
        &self,
        receiver: Receiver<Self::ProducerItem>,
        sender: Sender<Self::ConsumerItem>,
    ) -> Result<()> {
        let mut consumer_output = ConsumerOutputWcc::default();

        while let Ok(chunk) = receiver.recv() {
            for file in chunk {
                let path = Path::new(&file);
                let file_name = path
                    .file_name()
                    .ok_or(Error::PathConversion)?
                    .to_str()
                    .ok_or(Error::PathConversion)?
                    .into();

                // Get the coverage vector from the covdir file
                // If not present the file will be added to the files ignored
                let covdir_source_file = match self.covdir.source_files.get(&file) {
                    Some(source_file) => source_file,
                    None => {
                        let mut files_ignored = self.files_ignored.lock()?;
                        files_ignored.push(file);
                        continue;
                    }
                };

                let coverage = covdir_source_file.coverage.to_vec();
                let root = get_root(path)?;
                let coverage_percent = Some(covdir_source_file.coverage_percent);
                let ploc = root.metrics.loc.ploc();
                let comp = match self.metric {
                    Complexity::Cyclomatic => root.metrics.cyclomatic.cyclomatic_sum(),
                    Complexity::Cognitive => root.metrics.cognitive.cognitive_sum(),
                };
                let file_path = file.clone().split_off(self.prefix);

                // Upgrade all the global variables and add metrics to the result and complex_files
                let (m, (sp_sum, sq_sum)): (Metrics, (f64, f64)) = Tree::get_metrics_from_space(
                    &root,
                    &coverage
                        .iter()
                        .map(|c| Some(c.to_owned()))
                        .collect::<Vec<Option<i32>>>(),
                    self.metric,
                    coverage_percent,
                    &self.thresholds,
                )?;
                let mut files_metrics = self.files_metrics.lock()?;

                // Update all shared variables
                consumer_output.ploc_sum += ploc;
                consumer_output.wcc_plain_sum += sp_sum;
                consumer_output.wcc_quantized_sum += sq_sum;
                consumer_output.comp_sum += comp;
                files_metrics.push(FileMetrics::new(m, file_name, file_path));
            }
        }

        sender.send(consumer_output)?;

        Ok(())
    }

    fn composer(&self, receiver: Receiver<Self::ConsumerItem>) -> Result<Self::Output> {
        let mut consumers_total_output = ConsumerOutputWcc::default();
        while let Ok(consumer_output) = receiver.recv() {
            consumers_total_output.update(consumer_output);
        }

        let mut files_ignored = self.files_ignored.lock()?;
        let mut files_metrics = self.files_metrics.lock()?;
        let project_coverage = self.covdir.total_coverage;

        // Get final  metrics for all the project
        let project_metric = FileMetrics::new(
            get_project_metrics(consumers_total_output, Some(project_coverage))?,
            "PROJECT".into(),
            "-".into(),
        );
        files_ignored.sort();
        files_metrics.sort_by(|a, b| a.file.cmp(&b.file));

        // Get AVG MIN MAX and complex files
        let mut complex_files = files_metrics
            .iter()
            .filter(|m| m.metrics.is_complex)
            .cloned()
            .collect::<Vec<FileMetrics>>();
        complex_files.sort_by(|a, b| match self.sort_by {
            Sort::WccPlain => b.metrics.wcc_plain.total_cmp(&a.metrics.wcc_plain),
            Sort::WccQuantized => b.metrics.wcc_quantized.total_cmp(&a.metrics.wcc_quantized),
            Sort::Crap => b.metrics.crap.total_cmp(&a.metrics.crap),
            Sort::Skunk => b.metrics.skunk.total_cmp(&a.metrics.skunk),
        });

        let m = files_metrics
            .iter()
            .map(|metric| metric.metrics)
            .collect::<Vec<Metrics>>();

        let (avg, max, min) = get_cumulative_values(&m);
        files_metrics.push(project_metric);
        files_metrics.push(FileMetrics::avg(avg));
        files_metrics.push(FileMetrics::max(max));
        files_metrics.push(FileMetrics::min(min));

        Ok((
            (*files_metrics).clone(),
            (*files_ignored).clone(),
            complex_files,
            project_coverage,
        ))
    }
}
