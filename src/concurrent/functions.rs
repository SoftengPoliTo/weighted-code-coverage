use std::path::Path;
use std::sync::Mutex;

use crossbeam::channel::{Receiver, Sender};
use serde::{Deserialize, Serialize};
use tracing::debug;

use crate::grcov::covdir::Covdir;
use crate::grcov::coveralls::Coveralls;
use crate::utility::*;
use crate::{error::*, Complexity, Sort};

use super::{ConsumerOutputWcc, Metrics, Wcc};

// Struct with all the metrics computed for the root
#[derive(Clone, Default, Debug, Serialize, Deserialize, PartialEq)]
pub(crate) struct RootMetrics {
    pub(crate) metrics: Metrics,
    pub(crate) file_name: String,
    pub(crate) file_path: String,
    pub(crate) start_line: usize,
    pub(crate) end_line: usize,
    pub(crate) functions: Vec<FunctionMetrics>,
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
            file_name: "AVG".into(),
            file_path: "-".into(),
            start_line: 0,
            end_line: 0,
            functions: Vec::<FunctionMetrics>::new(),
        }
    }

    fn min(m: Metrics) -> Self {
        Self {
            metrics: m,
            file_name: "MIN".into(),
            file_path: "-".into(),
            start_line: 0,
            end_line: 0,
            functions: Vec::<FunctionMetrics>::new(),
        }
    }

    fn max(m: Metrics) -> Self {
        Self {
            metrics: m,
            file_name: "MAX".into(),
            file_path: "-".into(),
            start_line: 0,
            end_line: 0,
            functions: Vec::<FunctionMetrics>::new(),
        }
    }
}

// Struct with all the metrics computed for a single function
#[derive(Clone, Default, Debug, Serialize, Deserialize, PartialEq)]
pub(crate) struct FunctionMetrics {
    pub(crate) metrics: Metrics,
    pub(crate) function_name: String,
    pub(crate) function_path: String,
    pub(crate) start_line: usize,
    pub(crate) end_line: usize,
}

impl FunctionMetrics {
    fn new(
        metrics: Metrics,
        function_name: String,
        function_path: String,
        start_line: usize,
        end_line: usize,
    ) -> Self {
        Self {
            metrics,
            function_name,
            function_path,
            start_line,
            end_line,
        }
    }
}

pub(crate) struct CoverallsFunctionsWcc {
    pub(crate) chunks: Vec<Vec<String>>,
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
            functions_metrics: Mutex::new(Vec::<RootMetrics>::new()),
            sort_by,
        }
    }
}

impl Wcc for CoverallsFunctionsWcc {
    type ProducerItem = Vec<String>;
    type ConsumerItem = ConsumerOutputWcc;
    type Output = (Vec<RootMetrics>, Vec<String>, Vec<FunctionMetrics>, f64);

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
        let mut composer_output = ConsumerOutputWcc::default();

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
                    let (m, _): (Metrics, (f64, f64)) = Tree::get_metrics_from_space(
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

                let (m, (sp_sum, sq_sum)): (Metrics, (f64, f64)) = Tree::get_metrics_from_space(
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
                composer_output.wcc_quantized_sum += sq_sum;
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
            "PROJECT".into(),
            "-".into(),
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
    pub(crate) chunks: Vec<Vec<String>>,
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
            functions_metrics: Mutex::new(Vec::<RootMetrics>::new()),
            sort_by,
        }
    }
}

impl Wcc for CovdirFunctionsWcc {
    type ProducerItem = Vec<String>;
    type ConsumerItem = ConsumerOutputWcc;
    type Output = (Vec<RootMetrics>, Vec<String>, Vec<FunctionMetrics>, f64);

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
        let mut composer_output = ConsumerOutputWcc::default();

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
                    let (m, _): (Metrics, (f64, f64)) = Tree::get_metrics_from_space(
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

                // Upgrade all the global variables and add metrics to the result and complex_files
                let mut functions_metrics = self.functions_metrics.lock()?;
                composer_output.ploc_sum += ploc;
                composer_output.wcc_plain_sum += sp_sum;
                composer_output.wcc_quantized_sum += sq_sum;
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
            "PROJECT".into(),
            "-".into(),
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

#[cfg(test)]
mod tests {

    use super::*;
    use crate::{
        utility::{chunk_vector, compare_float, get_prefix},
        Complexity,
    };
    use std::fs;

    const JSON: &str = "./data/seahorse/seahorse.json";
    const COVDIR: &str = "./data/seahorse/covdir.json";
    const PROJECT: &str = "./data/seahorse/";
    const IGNORED: &str = "./data/seahorse/src/action.rs";

    fn get_test_data<P: AsRef<Path>>(
        files_path: P,
        json_path: P,
    ) -> (String, usize, Vec<Vec<String>>, String) {
        let files = read_files(files_path.as_ref()).unwrap();
        let json = fs::read_to_string(json_path).unwrap();
        let prefix = get_prefix(&files_path).unwrap();
        let chunks = chunk_vector(files, 8);
        let project_path = files_path.as_ref().to_str().unwrap().to_owned();

        (json, prefix, chunks, project_path)
    }

    #[test]
    fn test_metrics_coveralls_cyclomatic() {
        let json_path = Path::new(JSON);
        let project = Path::new(PROJECT);
        let ignored = Path::new(IGNORED);
        let (json, prefix, chunks, project_path) = get_test_data(project, json_path);
        let coveralls = Coveralls::new(json, project_path).unwrap();

        let (metrics, files_ignored, _, _) = CoverallsFunctionsWcc {
            chunks,
            coveralls,
            metric: Complexity::Cyclomatic,
            prefix,
            thresholds: vec![30., 1.5, 35., 30.],
            files_ignored: Mutex::new(Vec::new()),
            functions_metrics: Mutex::new(Vec::new()),
            sort_by: Sort::WccPlain,
        }
        .run(7)
        .unwrap();

        let ma = &metrics[7].metrics;
        let h = &metrics[5].metrics;
        let app_root = &metrics[0].metrics;
        let app_app_new_only_test = &metrics[0].functions[0].metrics;
        let cont_root = &metrics[2].metrics;
        let cont_bool_flag = &metrics[2].functions[3].metrics;

        assert_eq!(files_ignored.len(), 1);
        assert!(files_ignored[0] == ignored.as_os_str().to_str().unwrap());
        assert!(compare_float(ma.wcc_plain, 0.));
        assert!(compare_float(ma.wcc_quantized, 0.));
        assert!(compare_float(ma.crap, 552.));
        assert!(compare_float(ma.skunk, 92.));
        assert!(compare_float(h.wcc_plain, 1.5));
        assert!(compare_float(h.wcc_quantized, 0.5));
        assert!(compare_float(h.crap, 3.));
        assert!(compare_float(h.skunk, 0.));
        assert!(compare_float(app_root.wcc_plain, 79.21478060046189));
        assert!(compare_float(app_root.wcc_quantized, 0.792147806004619));
        assert!(compare_float(app_root.crap, 123.97408556537728));
        assert!(compare_float(app_root.skunk, 53.53535353535352));
        assert!(compare_float(cont_root.wcc_plain, 24.31578947368421));
        assert!(compare_float(cont_root.wcc_quantized, 0.7368421052631579));
        assert!(compare_float(cont_root.crap, 33.468144844401756));
        assert!(compare_float(cont_root.skunk, 9.9622641509434));
        assert!(compare_float(
            app_app_new_only_test.wcc_plain,
            1.1111111111111112
        ));
        assert!(compare_float(
            app_app_new_only_test.wcc_quantized,
            1.1111111111111112
        ));
        assert!(compare_float(app_app_new_only_test.crap, 1.0));
        assert!(compare_float(app_app_new_only_test.skunk, 0.000));
        assert!(compare_float(cont_bool_flag.wcc_plain, 2.142857142857143));
        assert!(compare_float(
            cont_bool_flag.wcc_quantized,
            0.7142857142857143
        ));
        assert!(compare_float(cont_bool_flag.crap, 3.0416666666666665));
        assert!(compare_float(cont_bool_flag.skunk, 1.999999999999999));
    }

    #[test]
    fn test_metrics_coveralls_cognitive() {
        let json_path = Path::new(JSON);
        let project = Path::new(PROJECT);
        let ignored = Path::new(IGNORED);
        let (json, prefix, chunks, project_path) = get_test_data(project, json_path);
        let coveralls = Coveralls::new(json, project_path).unwrap();

        let (metrics, files_ignored, _, _) = CoverallsFunctionsWcc {
            chunks,
            coveralls,
            metric: Complexity::Cognitive,
            prefix,
            thresholds: vec![30., 1.5, 35., 30.],
            files_ignored: Mutex::new(Vec::new()),
            functions_metrics: Mutex::new(Vec::new()),
            sort_by: Sort::WccPlain,
        }
        .run(7)
        .unwrap();

        let ma = &metrics[7].metrics;
        let h = &metrics[5].metrics;
        let app_root = &metrics[0].metrics;
        let app_app_new_only_test = &metrics[0].functions[0].metrics;
        let cont_root = &metrics[2].metrics;
        let cont_bool_flag = &metrics[2].functions[3].metrics;

        assert_eq!(files_ignored.len(), 1);
        assert!(files_ignored[0] == ignored.as_os_str().to_str().unwrap());
        assert!(compare_float(ma.wcc_plain, 0.));
        assert!(compare_float(ma.wcc_quantized, 0.));
        assert!(compare_float(ma.crap, 72.));
        assert!(compare_float(ma.skunk, 32.));
        assert!(compare_float(h.wcc_plain, 0.));
        assert!(compare_float(h.wcc_quantized, 0.5));
        assert!(compare_float(h.crap, 0.));
        assert!(compare_float(h.skunk, 0.));
        assert!(compare_float(app_root.wcc_plain, 66.540415704388));
        assert!(compare_float(app_root.wcc_quantized, 0.792147806004619));
        assert!(compare_float(app_root.crap, 100.91611477493021));
        assert!(compare_float(app_root.skunk, 44.969696969696955));
        assert!(compare_float(cont_root.wcc_plain, 18.42105263157895));
        assert!(compare_float(cont_root.wcc_quantized, 0.8872180451127819));
        assert!(compare_float(cont_root.crap, 25.268678170570336));
        assert!(compare_float(cont_root.skunk, 7.547169811320757));
        assert!(compare_float(app_app_new_only_test.wcc_plain, 0.0));
        assert!(compare_float(
            app_app_new_only_test.wcc_quantized,
            1.1111111111111112
        ));
        assert!(compare_float(app_app_new_only_test.crap, 0.0));
        assert!(compare_float(app_app_new_only_test.skunk, 0.000));
        assert!(compare_float(cont_bool_flag.wcc_plain, 0.7142857142857143));
        assert!(compare_float(
            cont_bool_flag.wcc_quantized,
            0.7142857142857143
        ));
        assert!(compare_float(cont_bool_flag.crap, 1.0046296296296295));
        assert!(compare_float(cont_bool_flag.skunk, 0.6666666666666663));
    }

    #[test]
    fn test_metrics_covdir_cyclomatic() {
        let json_path = Path::new(COVDIR);
        let project = Path::new(PROJECT);
        let ignored = Path::new(IGNORED);
        let (json, prefix, chunks, project_path) = get_test_data(project, json_path);
        let covdir = Covdir::new(json, project_path).unwrap();

        let (metrics, files_ignored, _, _) = CovdirFunctionsWcc {
            chunks,
            covdir,
            metric: Complexity::Cyclomatic,
            prefix,
            thresholds: vec![30., 1.5, 35., 30.],
            files_ignored: Mutex::new(Vec::new()),
            functions_metrics: Mutex::new(Vec::new()),
            sort_by: Sort::WccPlain,
        }
        .run(7)
        .unwrap();

        let ma = &metrics[7].metrics;
        let h = &metrics[5].metrics;
        let app_root = &metrics[0].metrics;
        let app_app_new_only_test = &metrics[0].functions[0].metrics;
        let cont_root = &metrics[2].metrics;
        let cont_bool_flag = &metrics[2].functions[3].metrics;

        assert_eq!(files_ignored.len(), 1);
        assert!(files_ignored[0] == ignored.as_os_str().to_str().unwrap());
        assert!(compare_float(ma.wcc_plain, 0.));
        assert!(compare_float(ma.wcc_quantized, 0.));
        assert!(compare_float(ma.crap, 552.));
        assert!(compare_float(ma.skunk, 92.));
        assert!(compare_float(h.wcc_plain, 1.5));
        assert!(compare_float(h.wcc_quantized, 0.5));
        assert!(compare_float(h.crap, 3.));
        assert!(compare_float(h.skunk, 0.));
        assert!(compare_float(app_root.wcc_plain, 79.21478060046189));
        assert!(compare_float(app_root.wcc_quantized, 0.792147806004619));
        assert!(compare_float(app_root.crap, 123.95346471999996));
        assert!(compare_float(app_root.skunk, 53.51999999999998));
        assert!(compare_float(cont_root.wcc_plain, 24.31578947368421));
        assert!(compare_float(cont_root.wcc_quantized, 0.7368421052631579));
        assert!(compare_float(cont_root.crap, 33.468671704875));
        assert!(compare_float(cont_root.skunk, 9.965999999999998));
        assert!(compare_float(
            app_app_new_only_test.wcc_plain,
            1.1111111111111112
        ));
        assert!(compare_float(
            app_app_new_only_test.wcc_quantized,
            1.1111111111111112
        ));
        assert!(compare_float(app_app_new_only_test.crap, 1.002395346472));
        assert!(compare_float(
            app_app_new_only_test.skunk,
            0.5351999999999998
        ));
        assert!(compare_float(cont_bool_flag.wcc_plain, 2.142857142857143));
        assert!(compare_float(
            cont_bool_flag.wcc_quantized,
            0.7142857142857143
        ));
        assert!(compare_float(cont_bool_flag.crap, 3.003873319875));
        assert!(compare_float(cont_bool_flag.skunk, 0.9059999999999996));
    }

    #[test]
    fn test_metrics_covdir_cognitive() {
        let json_path = Path::new(COVDIR);
        let project = Path::new(PROJECT);
        let ignored = Path::new(IGNORED);
        let (json, prefix, chunks, project_path) = get_test_data(project, json_path);
        let covdir = Covdir::new(json, project_path).unwrap();

        let (metrics, files_ignored, _, _) = CovdirFunctionsWcc {
            chunks,
            covdir,
            metric: Complexity::Cognitive,
            prefix,
            thresholds: vec![30., 1.5, 35., 30.],
            files_ignored: Mutex::new(Vec::new()),
            functions_metrics: Mutex::new(Vec::new()),
            sort_by: Sort::WccPlain,
        }
        .run(7)
        .unwrap();

        let ma = &metrics[7].metrics;
        let h = &metrics[5].metrics;
        let app_root = &metrics[0].metrics;
        let app_app_new_only_test = &metrics[0].functions[0].metrics;
        let cont_root = &metrics[2].metrics;
        let cont_bool_flag = &metrics[2].functions[3].metrics;

        assert_eq!(files_ignored.len(), 1);
        assert!(files_ignored[0] == ignored.as_os_str().to_str().unwrap());
        assert!(compare_float(ma.wcc_plain, 0.));
        assert!(compare_float(ma.wcc_quantized, 0.));
        assert!(compare_float(ma.crap, 72.));
        assert!(compare_float(ma.skunk, 32.));
        assert!(compare_float(h.wcc_plain, 0.));
        assert!(compare_float(h.wcc_quantized, 0.5));
        assert!(compare_float(h.crap, 0.));
        assert!(compare_float(h.skunk, 0.));
        assert!(compare_float(app_root.wcc_plain, 66.540415704388));
        assert!(compare_float(app_root.wcc_quantized, 0.792147806004619));
        assert!(compare_float(app_root.crap, 100.90156470643197));
        assert!(compare_float(app_root.skunk, 44.95679999999998));
        assert!(compare_float(cont_root.wcc_plain, 18.42105263157895));
        assert!(compare_float(cont_root.wcc_quantized, 0.8872180451127819));
        assert!(compare_float(cont_root.crap, 25.268980546875));
        assert!(compare_float(cont_root.skunk, 7.549999999999997));
        assert!(compare_float(app_app_new_only_test.wcc_plain, 0.0));
        assert!(compare_float(
            app_app_new_only_test.wcc_quantized,
            1.1111111111111112
        ));
        assert!(compare_float(app_app_new_only_test.crap, 0.0));
        assert!(compare_float(app_app_new_only_test.skunk, 0.000));
        assert!(compare_float(cont_bool_flag.wcc_plain, 0.7142857142857143));
        assert!(compare_float(
            cont_bool_flag.wcc_quantized,
            0.7142857142857143
        ));
        assert!(compare_float(cont_bool_flag.crap, 1.000430368875));
        assert!(compare_float(cont_bool_flag.skunk, 0.3019999999999999));
    }
}
