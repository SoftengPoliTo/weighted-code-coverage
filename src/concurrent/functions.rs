use std::path::Path;
use std::sync::Mutex;

use crossbeam::channel::{Receiver, Sender};
use rust_code_analysis::{FuncSpace, SpaceKind};
use serde::{Deserialize, Serialize};
use tracing::debug;

use crate::concurrent::Visit;
use crate::grcov::covdir::Covdir;
use crate::grcov::coveralls::Coveralls;
use crate::metrics::{get_covered_lines, get_root};
use crate::{error::*, Complexity, Sort};

use super::{get_cumulative_values, get_project_metrics, ConsumerOutputWcc, Metrics, Tree, Wcc};

/// Struct with all the metrics computed for the root
#[derive(Clone, Default, Debug, Serialize, Deserialize, PartialEq)]
pub struct RootMetrics {
    pub metrics: Metrics,
    pub file_name: String,
    pub file_path: String,
    pub start_line: usize,
    pub end_line: usize,
    pub functions: Vec<FunctionMetrics>,
}

impl RootMetrics {
    fn new(
        metrics: Metrics,
        file_name: String,
        file_path: String,
        start_line: usize,
        end_line: usize,
        functions: Vec<FunctionMetrics>,
    ) -> Self {
        Self {
            metrics,
            file_name,
            file_path,
            start_line,
            end_line,
            functions,
        }
    }

    fn avg(m: Metrics) -> Self {
        Self {
            metrics: m,
            file_name: "Average".into(),
            file_path: "Average".into(),
            start_line: 0,
            end_line: 0,
            functions: Vec::<FunctionMetrics>::new(),
        }
    }

    fn min(m: Metrics) -> Self {
        Self {
            metrics: m,
            file_name: "Min".into(),
            file_path: "Min".into(),
            start_line: 0,
            end_line: 0,
            functions: Vec::<FunctionMetrics>::new(),
        }
    }

    fn max(m: Metrics) -> Self {
        Self {
            metrics: m,
            file_name: "Max".into(),
            file_path: "Max".into(),
            start_line: 0,
            end_line: 0,
            functions: Vec::<FunctionMetrics>::new(),
        }
    }
}

/// Struct with all the metrics computed for a single function
#[derive(Clone, Default, Debug, Serialize, Deserialize, PartialEq)]
pub struct FunctionMetrics {
    pub metrics: Metrics,
    pub name: String,
    pub path: String,
    pub start_line: usize,
    pub end_line: usize,
}

impl FunctionMetrics {
    fn new(
        metrics: Metrics,
        name: String,
        path: String,
        start_line: usize,
        end_line: usize,
    ) -> Self {
        Self {
            metrics,
            name,
            path,
            start_line,
            end_line,
        }
    }
}

pub(crate) struct CoverallsFunctionsWcc {
    pub(crate) files: Vec<String>,
    pub(crate) coveralls: Coveralls,
    pub(crate) metric: Complexity,
    pub(crate) prefix: usize,
    pub(crate) thresholds: Vec<f64>,
    pub(crate) files_ignored: Mutex<Vec<String>>,
    pub(crate) functions_metrics: Mutex<Vec<RootMetrics>>,
    pub(crate) sort_by: Sort,
}

impl CoverallsFunctionsWcc {
    pub(crate) fn new(
        files: Vec<String>,
        coveralls: Coveralls,
        metric: Complexity,
        prefix: usize,
        thresholds: Vec<f64>,
        sort_by: Sort,
    ) -> Self {
        Self {
            files,
            coveralls,
            metric,
            prefix,
            thresholds,
            files_ignored: Mutex::new(Vec::<String>::new()),
            functions_metrics: Mutex::new(Vec::<RootMetrics>::new()),
            sort_by,
        }
    }
}

impl Wcc for CoverallsFunctionsWcc {
    type ProducerItem = String;
    type ConsumerItem = ConsumerOutputWcc;
    type Output = (Vec<RootMetrics>, Vec<String>, Vec<FunctionMetrics>, f64);

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
        let mut composer_output = ConsumerOutputWcc::default();

        while let Ok(file) = receiver.recv() {
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

            let spaces = get_spaces(&root)?;
            let ploc = root.metrics.loc.ploc();
            let comp = match self.metric {
                Complexity::Cyclomatic => root.metrics.cyclomatic.cyclomatic_sum(),
                Complexity::Cognitive => root.metrics.cognitive.cognitive_sum(),
            };

            let mut functions = Vec::<FunctionMetrics>::new();
            spaces.iter().try_for_each(|el| -> Result<()> {
                let space = el.0;
                let function_path = el.1.to_string();
                let (m, _): (Metrics, (f64, f64, f64, f64)) = Tree::get_metrics_from_space(
                    space,
                    &coverage,
                    self.metric,
                    None,
                    &self.thresholds,
                )?;
                let function_name = format!(
                    "{} ({}, {})",
                    space.name.as_ref().ok_or(Error::PathConversion)?,
                    space.start_line,
                    space.end_line
                );
                functions.push(FunctionMetrics::new(
                    m,
                    function_name,
                    function_path,
                    space.start_line,
                    space.end_line,
                ));
                Ok(())
            })?;

            let (m, (sp_sum, sp_max, sq_sum, sq_max)): (Metrics, (f64, f64, f64, f64)) =
                Tree::get_metrics_from_space(
                    &root,
                    &coverage,
                    self.metric,
                    None,
                    &self.thresholds,
                )?;
            let file_path = file.clone().split_off(self.prefix);

            // Update all the global variables and add metrics to the result and complex_files
            let mut functions_metrics = self.functions_metrics.lock()?;
            composer_output.covered_lines += covered_lines;
            composer_output.total_lines += tot_lines;
            composer_output.ploc_sum += ploc;
            composer_output.wcc_plain_sum += sp_sum;
            composer_output.wcc_plain_max += sp_max;
            composer_output.wcc_quantized_sum += sq_sum;
            composer_output.wcc_quantized_max += sq_max;
            composer_output.comp_sum += comp;
            functions_metrics.push(RootMetrics::new(
                m,
                file_name,
                file_path,
                root.start_line,
                root.end_line,
                functions,
            ));
        }

        sender.send(composer_output)?;

        Ok(())
    }

    fn composer(&self, receiver: Receiver<Self::ConsumerItem>) -> Result<Self::Output> {
        let mut consumers_total_output = ConsumerOutputWcc::default();
        while let Ok(consumer_output) = receiver.recv() {
            consumers_total_output.update(consumer_output);
        }

        let mut files_ignored = self.files_ignored.lock()?;
        let mut functions_metrics = self.functions_metrics.lock()?;

        // Get final  metrics for all the project
        let project_metric = RootMetrics::new(
            get_project_metrics(consumers_total_output, None)?,
            "Project".into(),
            "Project".into(),
            0,
            0,
            Vec::<FunctionMetrics>::new(),
        );

        let project_coverage = project_metric.metrics.coverage;
        files_ignored.sort();
        functions_metrics.sort_by(|a, b| a.file_name.cmp(&b.file_name));

        // Get AVG MIN MAX and complex files
        let mut complex_functions = functions_metrics
            .iter()
            .flat_map(|m| m.functions.clone())
            .filter(|m| m.metrics.is_complex)
            .collect::<Vec<FunctionMetrics>>();
        complex_functions.sort_by(|a, b| match self.sort_by {
            Sort::WccPlain => b.metrics.wcc_plain.total_cmp(&a.metrics.wcc_plain),
            Sort::WccQuantized => b.metrics.wcc_quantized.total_cmp(&a.metrics.wcc_quantized),
            Sort::Crap => b.metrics.crap.total_cmp(&a.metrics.crap),
            Sort::Skunk => b.metrics.skunk.total_cmp(&a.metrics.skunk),
        });

        let m = functions_metrics
            .iter()
            .map(|metric| metric.metrics)
            .collect::<Vec<Metrics>>();

        let (avg, max, min) = get_cumulative_values(&m);
        functions_metrics.push(project_metric);
        functions_metrics.push(RootMetrics::avg(avg));
        functions_metrics.push(RootMetrics::max(max));
        functions_metrics.push(RootMetrics::min(min));

        Ok((
            (*functions_metrics).clone(),
            (*files_ignored).clone(),
            complex_functions,
            f64::round(project_coverage * 100.) / 100.,
        ))
    }
}

pub(crate) struct CovdirFunctionsWcc {
    pub(crate) files: Vec<String>,
    pub(crate) covdir: Covdir,
    pub(crate) metric: Complexity,
    pub(crate) prefix: usize,
    pub(crate) thresholds: Vec<f64>,
    pub(crate) files_ignored: Mutex<Vec<String>>,
    pub(crate) functions_metrics: Mutex<Vec<RootMetrics>>,
    pub(crate) sort_by: Sort,
}

impl CovdirFunctionsWcc {
    pub(crate) fn new(
        files: Vec<String>,
        covdir: Covdir,
        metric: Complexity,
        prefix: usize,
        thresholds: Vec<f64>,
        sort_by: Sort,
    ) -> Self {
        Self {
            files,
            covdir,
            metric,
            prefix,
            thresholds,
            files_ignored: Mutex::new(Vec::<String>::new()),
            functions_metrics: Mutex::new(Vec::<RootMetrics>::new()),
            sort_by,
        }
    }
}

impl Wcc for CovdirFunctionsWcc {
    type ProducerItem = String;
    type ConsumerItem = ConsumerOutputWcc;
    type Output = (Vec<RootMetrics>, Vec<String>, Vec<FunctionMetrics>, f64);

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
        let mut composer_output = ConsumerOutputWcc::default();

        while let Ok(file) = receiver.recv() {
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
            let coverage_percent = Some(covdir_source_file.coverage_percent);
            let root = get_root(path)?;
            let spaces = get_spaces(&root)?;
            let ploc = root.metrics.loc.ploc();
            let comp = match self.metric {
                Complexity::Cyclomatic => root.metrics.cyclomatic.cyclomatic_sum(),
                Complexity::Cognitive => root.metrics.cognitive.cognitive_sum(),
            };

            let mut functions = Vec::<FunctionMetrics>::new();
            spaces.iter().try_for_each(|el| -> Result<()> {
                let space = el.0;
                let function_path = el.1.to_string();
                let function_name = format!(
                    "{} ({}, {})",
                    space.name.as_ref().ok_or(Error::Conversion)?,
                    space.start_line,
                    space.end_line
                );
                let (m, _): (Metrics, (f64, f64, f64, f64)) = Tree::get_metrics_from_space(
                    space,
                    &coverage
                        .iter()
                        .map(|c| Some(c.to_owned()))
                        .collect::<Vec<Option<i32>>>(),
                    self.metric,
                    coverage_percent,
                    &self.thresholds,
                )?;
                functions.push(FunctionMetrics::new(
                    m,
                    function_name,
                    function_path,
                    space.start_line,
                    space.end_line,
                ));
                Ok(())
            })?;
            let file_path = file.clone().split_off(self.prefix);

            let (m, (sp_sum, sp_max, sq_sum, sq_max)): (Metrics, (f64, f64, f64, f64)) =
                Tree::get_metrics_from_space(
                    &root,
                    &coverage
                        .iter()
                        .map(|c| Some(c.to_owned()))
                        .collect::<Vec<Option<i32>>>(),
                    self.metric,
                    coverage_percent,
                    &self.thresholds,
                )?;

            // Upgrade all the global variables and add metrics to the result and complex_files
            let mut functions_metrics = self.functions_metrics.lock()?;
            composer_output.ploc_sum += ploc;
            composer_output.wcc_plain_sum += sp_sum;
            composer_output.wcc_plain_max += sp_max;
            composer_output.wcc_quantized_sum += sq_sum;
            composer_output.wcc_quantized_max += sq_max;
            composer_output.comp_sum += comp;
            functions_metrics.push(RootMetrics::new(
                m,
                file_name,
                file_path,
                root.start_line,
                root.end_line,
                functions,
            ));
        }

        sender.send(composer_output)?;

        Ok(())
    }

    fn composer(&self, receiver: Receiver<Self::ConsumerItem>) -> Result<Self::Output> {
        let mut consumers_total_output = ConsumerOutputWcc::default();
        while let Ok(consumer_output) = receiver.recv() {
            consumers_total_output.update(consumer_output);
        }

        let mut files_ignored = self.files_ignored.lock()?;
        let mut functions_metrics = self.functions_metrics.lock()?;
        let project_coverage = self.covdir.total_coverage;

        // Get final  metrics for all the project
        let project_metric = RootMetrics::new(
            get_project_metrics(consumers_total_output, Some(project_coverage))?,
            "Project".into(),
            "Project".into(),
            0,
            0,
            Vec::<FunctionMetrics>::new(),
        );
        files_ignored.sort();
        functions_metrics.sort_by(|a, b| a.file_name.cmp(&b.file_name));

        // Get AVG MIN MAX and complex files
        let mut complex_functions = functions_metrics
            .iter()
            .flat_map(|m| m.functions.clone())
            .filter(|m| m.metrics.is_complex)
            .collect::<Vec<FunctionMetrics>>();
        complex_functions.sort_by(|a, b| match self.sort_by {
            Sort::WccPlain => b.metrics.wcc_plain.total_cmp(&a.metrics.wcc_plain),
            Sort::WccQuantized => b.metrics.wcc_quantized.total_cmp(&a.metrics.wcc_quantized),
            Sort::Crap => b.metrics.crap.total_cmp(&a.metrics.crap),
            Sort::Skunk => b.metrics.skunk.total_cmp(&a.metrics.skunk),
        });

        let m = functions_metrics
            .iter()
            .map(|metric| metric.metrics)
            .collect::<Vec<Metrics>>();

        let (avg, max, min) = get_cumulative_values(&m);
        functions_metrics.push(project_metric);
        functions_metrics.push(RootMetrics::avg(avg));
        functions_metrics.push(RootMetrics::max(max));
        functions_metrics.push(RootMetrics::min(min));

        Ok((
            (*functions_metrics).clone(),
            (*files_ignored).clone(),
            complex_functions,
            f64::round(project_coverage * 100.) / 100.,
        ))
    }
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
