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

#[cfg(test)]
mod tests {

    use super::*;
    use crate::{
        utility::{chunk_vector, compare_float, get_prefix},
        Complexity, Sort,
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

        let (metrics, files_ignored, _, _) = CoverallsFilesWcc {
            chunks,
            coveralls,
            metric: Complexity::Cyclomatic,
            prefix,
            thresholds: vec![30., 1.5, 35., 30.],
            files_ignored: Mutex::new(Vec::new()),
            files_metrics: Mutex::new(Vec::new()),
            sort_by: Sort::WccPlain,
        }
        .run(7)
        .unwrap();

        let error = &metrics[3].metrics;
        let ma = &metrics[7].metrics;
        let h = &metrics[5].metrics;
        let app = &metrics[0].metrics;
        let cont = &metrics[2].metrics;

        assert_eq!(files_ignored.len(), 1);
        assert!(files_ignored[0] == ignored.as_os_str().to_str().unwrap());
        assert!(compare_float(error.wcc_plain, 0.53125));
        assert!(compare_float(error.wcc_quantized, 0.03125));
        assert!(compare_float(error.crap, 257.94117647058823));
        assert!(compare_float(error.skunk, 64.00000000000001));
        assert!(compare_float(ma.wcc_plain, 0.));
        assert!(compare_float(ma.wcc_quantized, 0.));
        assert!(compare_float(ma.crap, 552.));
        assert!(compare_float(ma.skunk, 92.));
        assert!(compare_float(h.wcc_plain, 1.5));
        assert!(compare_float(h.wcc_quantized, 0.5));
        assert!(compare_float(h.crap, 3.));
        assert!(compare_float(h.skunk, 0.));
        assert!(compare_float(app.wcc_plain, 79.21478060046189));
        assert!(compare_float(app.wcc_quantized, 0.792147806004619));
        assert!(compare_float(app.crap, 123.97408556537728));
        assert!(compare_float(app.skunk, 53.53535353535352));
        assert!(compare_float(cont.wcc_plain, 24.31578947368421));
        assert!(compare_float(cont.wcc_quantized, 0.7368421052631579));
        assert!(compare_float(cont.crap, 33.468144844401756));
        assert!(compare_float(cont.skunk, 9.9622641509434));
    }

    #[test]
    fn test_metrics_coveralls_cognitive() {
        let json_path = Path::new(JSON);
        let project = Path::new(PROJECT);
        let ignored = Path::new(IGNORED);
        let (json, prefix, chunks, project_path) = get_test_data(project, json_path);
        let coveralls = Coveralls::new(json, project_path).unwrap();

        let (metrics, files_ignored, _, _) = CoverallsFilesWcc {
            chunks,
            coveralls,
            metric: Complexity::Cognitive,
            prefix,
            thresholds: vec![30., 1.5, 35., 30.],
            files_ignored: Mutex::new(Vec::new()),
            files_metrics: Mutex::new(Vec::new()),
            sort_by: Sort::WccPlain,
        }
        .run(7)
        .unwrap();

        let error = &metrics[3].metrics;
        let ma = &metrics[7].metrics;
        let h = &metrics[5].metrics;
        let app = &metrics[0].metrics;
        let cont = &metrics[2].metrics;

        assert_eq!(files_ignored.len(), 1);
        assert!(files_ignored[0] == ignored.as_os_str().to_str().unwrap());
        assert!(compare_float(error.wcc_plain, 0.0625));
        assert!(compare_float(error.wcc_quantized, 0.03125));
        assert!(compare_float(error.crap, 5.334825971911256));
        assert!(compare_float(error.skunk, 7.529411764705883));
        assert!(compare_float(ma.wcc_plain, 0.));
        assert!(compare_float(ma.wcc_quantized, 0.));
        assert!(compare_float(ma.crap, 72.));
        assert!(compare_float(ma.skunk, 32.));
        assert!(compare_float(h.wcc_plain, 0.));
        assert!(compare_float(h.wcc_quantized, 0.5));
        assert!(compare_float(h.crap, 0.));
        assert!(compare_float(h.skunk, 0.));
        assert!(compare_float(app.wcc_plain, 66.540415704388));
        assert!(compare_float(app.wcc_quantized, 0.792147806004619));
        assert!(compare_float(app.crap, 100.91611477493021));
        assert!(compare_float(app.skunk, 44.969696969696955));
        assert!(compare_float(cont.wcc_plain, 18.42105263157895));
        assert!(compare_float(cont.wcc_quantized, 0.8872180451127819));
        assert!(compare_float(cont.crap, 25.268678170570336));
        assert!(compare_float(cont.skunk, 7.547169811320757));
    }

    #[test]
    fn test_metrics_covdir_cyclomatic() {
        let json_path = Path::new(COVDIR);
        let project = Path::new(PROJECT);
        let ignored = Path::new(IGNORED);
        let (json, prefix, chunks, project_path) = get_test_data(project, json_path);
        let covdir = Covdir::new(json, project_path).unwrap();

        let (metrics, files_ignored, _, _) = CovdirFilesWcc {
            chunks,
            covdir,
            metric: Complexity::Cyclomatic,
            prefix,
            thresholds: vec![30., 1.5, 35., 30.],
            files_ignored: Mutex::new(Vec::new()),
            files_metrics: Mutex::new(Vec::new()),
            sort_by: Sort::WccPlain,
        }
        .run(7)
        .unwrap();

        let error = &metrics[3].metrics;
        let ma = &metrics[7].metrics;
        let h = &metrics[5].metrics;
        let app = &metrics[0].metrics;
        let cont = &metrics[2].metrics;

        assert_eq!(files_ignored.len(), 1);
        assert!(files_ignored[0] == ignored.as_os_str().to_str().unwrap());
        assert!(compare_float(error.wcc_plain, 0.53125));
        assert!(compare_float(error.wcc_quantized, 0.03125));
        assert!(compare_float(error.crap, 257.95924751059204));
        assert!(compare_float(error.skunk, 64.00160000000001));
        assert!(compare_float(ma.wcc_plain, 0.));
        assert!(compare_float(ma.wcc_quantized, 0.));
        assert!(compare_float(ma.crap, 552.));
        assert!(compare_float(ma.skunk, 92.));
        assert!(compare_float(h.wcc_plain, 1.5));
        assert!(compare_float(h.wcc_quantized, 0.5));
        assert!(compare_float(h.crap, 3.));
        assert!(compare_float(h.skunk, 0.));
        assert!(compare_float(app.wcc_plain, 79.21478060046189));
        assert!(compare_float(app.wcc_quantized, 0.792147806004619));
        assert!(compare_float(app.crap, 123.95346471999996));
        assert!(compare_float(app.skunk, 53.51999999999998));
        assert!(compare_float(cont.wcc_plain, 24.31578947368421));
        assert!(compare_float(cont.wcc_quantized, 0.7368421052631579));
        assert!(compare_float(cont.crap, 33.468671704875));
        assert!(compare_float(cont.skunk, 9.965999999999998));
    }

    #[test]
    fn test_metrics_covdir_cognitive() {
        let json_path = Path::new(COVDIR);
        let project = Path::new(PROJECT);
        let ignored = Path::new(IGNORED);
        let (json, prefix, chunks, project_path) = get_test_data(project, json_path);
        let covdir = Covdir::new(json, project_path).unwrap();

        let (metrics, files_ignored, _, _) = CovdirFilesWcc {
            chunks,
            covdir,
            metric: Complexity::Cognitive,
            prefix,
            thresholds: vec![30., 1.5, 35., 30.],
            files_ignored: Mutex::new(Vec::new()),
            files_metrics: Mutex::new(Vec::new()),
            sort_by: Sort::WccPlain,
        }
        .run(7)
        .unwrap();

        let error = &metrics[3].metrics;
        let ma = &metrics[7].metrics;
        let h = &metrics[5].metrics;
        let app = &metrics[0].metrics;
        let cont = &metrics[2].metrics;

        assert_eq!(files_ignored.len(), 1);
        assert!(files_ignored[0] == ignored.as_os_str().to_str().unwrap());
        assert!(compare_float(error.wcc_plain, 0.0625));
        assert!(compare_float(error.wcc_quantized, 0.03125));
        assert!(compare_float(error.crap, 5.3350760901120005));
        assert!(compare_float(error.skunk, 7.5296));
        assert!(compare_float(ma.wcc_plain, 0.));
        assert!(compare_float(ma.wcc_quantized, 0.));
        assert!(compare_float(ma.crap, 72.));
        assert!(compare_float(ma.skunk, 32.));
        assert!(compare_float(h.wcc_plain, 0.));
        assert!(compare_float(h.wcc_quantized, 0.5));
        assert!(compare_float(h.crap, 0.));
        assert!(compare_float(h.skunk, 0.));
        assert!(compare_float(app.wcc_plain, 66.540415704388));
        assert!(compare_float(app.wcc_quantized, 0.792147806004619));
        assert!(compare_float(app.crap, 100.90156470643197));
        assert!(compare_float(app.skunk, 44.95679999999998));
        assert!(compare_float(cont.wcc_plain, 18.42105263157895));
        assert!(compare_float(cont.wcc_quantized, 0.8872180451127819));
        assert!(compare_float(cont.crap, 25.268980546875));
        assert!(compare_float(cont.skunk, 7.549999999999997));
    }
}
