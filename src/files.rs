use std::collections::HashMap;
use std::fmt;
use std::fs;
use std::path::*;
use std::sync::{Arc, Mutex};
use std::thread;

use crossbeam::channel::{unbounded, Receiver, Sender};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tracing::debug;

use crate::error::*;
use crate::utility::*;

/// Struct containing all the metrics
#[derive(Clone, Default, Debug, Serialize, Deserialize, Copy, PartialEq)]
pub struct Metrics {
    pub wcc_plain: f64,
    pub wcc_quantized: f64,
    pub crap: f64,
    pub skunk: f64,
    pub is_complex: bool,
    pub coverage: f64,
}

impl Metrics {
    pub fn new(
        wcc_plain: f64,
        wcc_quantized: f64,
        crap: f64,
        skunk: f64,
        is_complex: bool,
        coverage: f64,
    ) -> Self {
        Self {
            wcc_plain,
            wcc_quantized,
            crap,
            skunk,
            is_complex,
            coverage,
        }
    }

    pub fn min() -> Self {
        Self {
            wcc_plain: f64::MAX,
            wcc_quantized: f64::MAX,
            crap: f64::MAX,
            skunk: f64::MAX,
            is_complex: false,
            coverage: 100.0,
        }
    }

    pub fn wcc_plain(mut self, wcc_plain: f64) -> Self {
        self.wcc_plain = wcc_plain;
        self
    }
    pub fn wcc_quantized(mut self, wcc_quantized: f64) -> Self {
        self.wcc_quantized = wcc_quantized;
        self
    }
    pub fn crap(mut self, crap: f64) -> Self {
        self.crap = crap;
        self
    }
    pub fn skunk(mut self, skunk: f64) -> Self {
        self.skunk = skunk;
        self
    }
    pub fn is_complex(mut self, is_complex: bool) -> Self {
        self.is_complex = is_complex;
        self
    }
    pub fn coverage(mut self, coverage: f64) -> Self {
        self.coverage = coverage;
        self
    }
}

/// Struct with all the metrics computed for a single file
#[derive(Clone, Default, Debug, Serialize, Deserialize, PartialEq)]
#[allow(dead_code)]
pub struct FileMetrics {
    pub metrics: Metrics,
    pub file: String,
    pub file_path: String,
}

impl FileMetrics {
    pub fn new(metrics: Metrics, file: String, file_path: String) -> Self {
        Self {
            metrics,
            file,
            file_path,
        }
    }

    pub fn avg(m: Metrics) -> Self {
        Self {
            metrics: m,
            file: "AVG".into(),
            file_path: "-".into(),
        }
    }

    pub fn min(m: Metrics) -> Self {
        Self {
            metrics: m,
            file: "MIN".into(),
            file_path: "-".into(),
        }
    }

    pub fn max(m: Metrics) -> Self {
        Self {
            metrics: m,
            file: "MAX".into(),
            file_path: "-".into(),
        }
    }
}

type Output = (Vec<FileMetrics>, Vec<String>, Vec<FileMetrics>, f64);

// Job received by the consumer threads
#[derive(Clone)]
struct JobItem {
    chunk: Vec<String>,
    covs: HashMap<String, Vec<Value>>,
    metric: Complexity,
    prefix: usize,
    thresholds: Vec<f64>,
}
impl JobItem {
    fn new(
        chunk: Vec<String>,
        covs: HashMap<String, Vec<Value>>,
        metric: Complexity,
        prefix: usize,
        thresholds: Vec<f64>,
    ) -> Self {
        Self {
            chunk,
            covs,
            metric,
            prefix,
            thresholds,
        }
    }
}

impl fmt::Debug for JobItem {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "Job: chunks:{:?}, metric:{}, prefix:{:?}, thresholds: {:?}",
            self.chunk, self.metric, self.prefix, self.thresholds
        )
    }
}

#[derive(Clone, Copy, Default)]
pub(crate) struct JobComposer {
    pub(crate) covered_lines: f64,
    pub(crate) total_lines: f64,
    pub(crate) wcc_plain_sum: f64,
    pub(crate) wcc_quantized_sum: f64,
    pub(crate) ploc_sum: f64,
    pub(crate) comp_sum: f64,
}
pub(crate) type ComposerReceiver = Receiver<Option<JobComposer>>;
pub(crate) type ComposerSender = Sender<Option<JobComposer>>;

pub(crate) fn composer(receiver: ComposerReceiver) -> Result<JobComposer> {
    let mut covered_lines = 0.0;
    let mut total_lines = 0.0;
    let mut wcc_plain_sum = 0.0;
    let mut wcc_quantized_sum = 0.0;
    let mut ploc_sum = 0.0;
    let mut comp_sum = 0.0;
    while let Ok(job) = receiver.recv() {
        if job.is_none() {
            break;
        }
        let job = job.unwrap();
        covered_lines += job.covered_lines;
        total_lines += job.total_lines;
        wcc_plain_sum += job.wcc_plain_sum;
        wcc_quantized_sum += job.wcc_quantized_sum;
        ploc_sum += job.ploc_sum;
        comp_sum += job.comp_sum;
    }
    Ok(JobComposer {
        covered_lines,
        total_lines,
        wcc_plain_sum,
        wcc_quantized_sum,
        ploc_sum,
        comp_sum,
    })
}

// Configuration shared by all threads with all the data that must be returned
#[derive(Clone, Default, Debug)]
pub struct Config {
    pub(crate) res: Arc<Mutex<Vec<FileMetrics>>>,
    pub(crate) files_ignored: Arc<Mutex<Vec<String>>>,
}

impl Config {
    fn new() -> Self {
        Self {
            res: Arc::new(Mutex::new(Vec::<FileMetrics>::new())),
            files_ignored: Arc::new(Mutex::new(Vec::<String>::new())),
        }
    }
    fn clone(&self) -> Self {
        Self {
            res: Arc::clone(&self.res),
            files_ignored: Arc::clone(&self.files_ignored),
        }
    }
}

type JobReceiver = Receiver<Option<JobItem>>;

// Consumer function run by ead independent thread
fn consumer(receiver: JobReceiver, sender_composer: ComposerSender, cfg: &Config) -> Result<()> {
    // Get all shared data
    let files_ignored = &cfg.files_ignored;
    let res = &cfg.res;
    let mut composer_output: JobComposer = JobComposer::default();
    while let Ok(job) = receiver.recv() {
        if job.is_none() {
            break;
        }
        // Cannot panic because of the check immediately above.
        let job = job.unwrap();
        let chunk = job.chunk;
        let covs = job.covs;
        let metric = job.metric;
        let prefix = job.prefix;
        let thresholds = job.thresholds;
        // For each file in the chunk received
        for file in chunk {
            let path = Path::new(&file);
            let file_name = path
                .file_name()
                .ok_or(Error::PathConversionError())?
                .to_str()
                .ok_or(Error::PathConversionError())?
                .into();
            // Get the coverage vector from the coveralls file
            // if not present the file will be added to the files ignored
            let arr = match covs.get(&file) {
                Some(arr) => arr.to_vec(),
                None => {
                    let mut f = files_ignored.lock()?;
                    f.push(file);
                    continue;
                }
            };
            let root = get_root(path)?;
            let (covered_lines, tot_lines) =
                get_covered_lines(&arr, root.start_line, root.end_line)?;
            debug!(
                "File: {:?} covered lines: {}  total lines: {}",
                file, covered_lines, tot_lines
            );
            let ploc = root.metrics.loc.ploc();
            let comp = match metric {
                Complexity::Cyclomatic => root.metrics.cyclomatic.cyclomatic_sum(),
                Complexity::Cognitive => root.metrics.cognitive.cognitive_sum(),
            };
            let file_path = file.clone().split_off(prefix);
            // Upgrade all the global variables and add metrics to the result and complex_files
            let (m, (sp_sum, sq_sum)): (Metrics, (f64, f64)) =
                Tree::get_metrics_from_space(&root, &arr, metric, None, &thresholds)?;
            let mut res = res.lock()?;
            composer_output.covered_lines += covered_lines;
            composer_output.total_lines += tot_lines;
            composer_output.ploc_sum += ploc;
            composer_output.wcc_plain_sum += sp_sum;
            composer_output.wcc_quantized_sum += sq_sum;
            composer_output.comp_sum += comp;
            res.push(FileMetrics::new(m, file_name, file_path));
        }
    }
    if let Err(_e) = sender_composer.send(Some(composer_output)) {
        println!("{_e}");
        return Err(Error::SenderError());
    }
    Ok(())
}

// Chunks the vector of files in multiple chunk to be used by threads
// It will return number of chunk with the same number of elements usually equal
// Or very close to n_threads
fn chunk_vector(vec: Vec<String>, n_threads: usize) -> Vec<Vec<String>> {
    let chunks = vec.chunks((vec.len() / n_threads).max(1));
    chunks
        .map(|chunk| chunk.iter().map(|c| c.into()).collect::<Vec<String>>())
        .collect::<Vec<Vec<String>>>()
}

/// This Function get the folder of the repo to analyzed and the path to the coveralls file obtained using grcov
/// It also takes as arguments the complexity metrics that must be used between cognitive or cyclomatic
/// If the a file is not found in the json that files will be skipped
/// It returns the  tuple (res, files_ignored, complex_files, project_coverage)
pub fn get_metrics_concurrent<A: AsRef<Path>, B: AsRef<Path>>(
    files_path: A,
    json_path: B,
    metric: Complexity,
    n_threads: usize,
    thresholds: &[f64],
    sort_by: Sort,
) -> Result<Output> {
    if thresholds.len() != 4 {
        return Err(Error::ThresholdsError());
    }
    // Take all the files starting from the given project folder
    let vec = read_files(files_path.as_ref())?;
    // Read coveralls file to string and then get all the coverage vectors
    let file = fs::read_to_string(json_path)?;
    let covs = read_json(
        file,
        files_path
            .as_ref()
            .to_str()
            .ok_or(Error::PathConversionError())?,
    )?;
    let mut handlers = vec![];
    // Create a new vonfig with  all needed mutexes
    let cfg = Config::new();
    let (sender, receiver) = unbounded();
    let (sender_composer, receiver_composer) = unbounded();
    // Chunks the files vector
    let chunks = chunk_vector(vec, n_threads);
    debug!("Files divided in {} chunks", chunks.len());
    debug!("Launching all {} threads", n_threads);
    let composer =
        { thread::spawn(move || -> Result<JobComposer> { composer(receiver_composer) }) };
    for _ in 0..n_threads {
        let s = sender_composer.clone();
        let r = receiver.clone();
        let config = cfg.clone();
        // Launch n_threads consume threads
        let h = thread::spawn(move || -> Result<()> { consumer(r, s, &config) });
        handlers.push(h);
    }
    let prefix = files_path
        .as_ref()
        .to_str()
        .ok_or(Error::PathConversionError())?
        .to_string()
        .len();
    // Send all chunks to the consumers
    chunks
        .iter()
        .try_for_each(|chunk: &Vec<String>| -> Result<()> {
            let job = JobItem::new(
                chunk.to_vec(),
                covs.clone(),
                metric,
                prefix,
                thresholds.to_vec(),
            );
            debug!("Sending job: {:?}", job);
            if let Err(_e) = sender.send(Some(job)) {
                return Err(Error::SenderError());
            }
            Ok(())
        })?;
    // Stops all consumers by poisoning them
    debug!("Poisoning Threads...");
    handlers.iter().try_for_each(|_| {
        if let Err(_e) = sender.send(None) {
            return Err(Error::SenderError());
        }
        Ok(())
    })?;
    // Wait the all consumers are  finished
    debug!("Waiting threads to finish...");
    for handle in handlers {
        handle.join()??;
    }
    if let Err(_e) = sender_composer.send(None) {
        return Err(Error::SenderError());
    }
    let mut files_ignored = cfg.files_ignored.lock()?;
    let mut res = cfg.res.lock()?;
    let composer_output = composer.join()??;
    let project_metric = FileMetrics::new(
        get_project_metrics(composer_output, None)?,
        "PROJECT".into(),
        "-".into(),
    );
    let project_coverage = project_metric.metrics.coverage;
    files_ignored.sort();
    res.sort_by(|a, b| a.file.cmp(&b.file));
    // Get AVG MIN MAX and complex files
    let mut complex_files = res
        .iter()
        .filter(|m| m.metrics.is_complex)
        .cloned()
        .collect::<Vec<FileMetrics>>();
    complex_files.sort_by(|a, b| match sort_by {
        Sort::WccPlain => b.metrics.wcc_plain.total_cmp(&a.metrics.wcc_plain),
        Sort::WccQuantized => b.metrics.wcc_quantized.total_cmp(&a.metrics.wcc_quantized),
        Sort::Crap => b.metrics.crap.total_cmp(&a.metrics.crap),
        Sort::Skunk => b.metrics.skunk.total_cmp(&a.metrics.skunk),
    });
    let m = res
        .iter()
        .map(|metric| metric.metrics)
        .collect::<Vec<Metrics>>();
    let (avg, max, min) = get_cumulative_values(&m);
    res.push(project_metric);
    res.push(FileMetrics::avg(avg));
    res.push(FileMetrics::max(max));
    res.push(FileMetrics::min(min));
    Ok((
        (*res).clone(),
        (*files_ignored).clone(),
        complex_files,
        f64::round(project_coverage * 100.) / 100.,
    ))
}

// Job received by the consumer threads for the covdir version
struct JobItemCovDir {
    chunk: Vec<String>,
    covs: HashMap<String, Covdir>,
    metric: Complexity,
    prefix: usize,
    thresholds: Vec<f64>,
}

impl JobItemCovDir {
    fn new(
        chunk: Vec<String>,
        covs: HashMap<String, Covdir>,
        metric: Complexity,
        prefix: usize,
        thresholds: Vec<f64>,
    ) -> Self {
        Self {
            chunk,
            covs,
            metric,
            prefix,
            thresholds,
        }
    }
}
impl fmt::Debug for JobItemCovDir {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "Job: chunks:{:?}, metric:{}, prefix:{:?}, thresholds: {:?}",
            self.chunk, self.metric, self.prefix, self.thresholds
        )
    }
}

type JobReceiverCovDir = Receiver<Option<JobItemCovDir>>;

// Consumer thread for the covdir format
fn consumer_covdir(
    receiver: JobReceiverCovDir,
    sender_composer: ComposerSender,
    cfg: &Config,
) -> Result<()> {
    // Get all shared variables
    let files_ignored = &cfg.files_ignored;
    let res = &cfg.res;
    let mut composer_output = JobComposer::default();
    while let Ok(job) = receiver.recv() {
        if job.is_none() {
            break;
        }
        // Cannot panic because of the check immediately above.
        let job = job.unwrap();
        let chunk = job.chunk;
        let covs = job.covs;
        let metric = job.metric;
        let prefix = job.prefix;
        let thresholds = job.thresholds;
        // For each file in the chunk
        for file in chunk {
            let path = Path::new(&file);
            let file_name = path
                .file_name()
                .ok_or(Error::PathConversionError())?
                .to_str()
                .ok_or(Error::PathConversionError())?
                .into();
            // Get the coverage vector from the covdir file
            // If not present the file will be added to the files ignored
            let covdir = match covs.get(&file) {
                Some(covdir) => covdir,
                None => {
                    let mut f = files_ignored.lock()?;
                    f.push(file);
                    continue;
                }
            };
            let arr = &covdir.arr;
            let coverage = Some(covdir.coverage);
            let root = get_root(path)?;
            let ploc = root.metrics.loc.ploc();
            let comp = match metric {
                Complexity::Cyclomatic => root.metrics.cyclomatic.cyclomatic_sum(),
                Complexity::Cognitive => root.metrics.cognitive.cognitive_sum(),
            };
            let file_path = file.clone().split_off(prefix);
            let (m, (sp_sum, sq_sum)): (Metrics, (f64, f64)) =
                Tree::get_metrics_from_space(&root, arr, metric, coverage, &thresholds)?;
            let mut res = res.lock()?;
            // Update all shared variables
            composer_output.ploc_sum += ploc;
            composer_output.wcc_plain_sum += sp_sum;
            composer_output.wcc_quantized_sum += sq_sum;
            composer_output.comp_sum += comp;
            res.push(FileMetrics::new(m, file_name, file_path));
        }
    }
    if let Err(_e) = sender_composer.send(Some(composer_output)) {
        return Err(Error::SenderError());
    }
    Ok(())
}

/// This Function get the folder of the repo to analyzed and the path to the covdir file obtained using grcov
/// It also takes as arguments the complexity metrics that must be used between cognitive or cyclomatic
/// If the a file is not found in the json that files will be skipped
/// It returns the  tuple (res, files_ignored, complex_files, project_coverage)
pub fn get_metrics_concurrent_covdir<A: AsRef<Path>, B: AsRef<Path>>(
    files_path: A,
    json_path: B,
    metric: Complexity,
    n_threads: usize,
    thresholds: &[f64],
    sort_by: Sort,
) -> Result<Output> {
    if thresholds.len() != 4 {
        return Err(Error::ThresholdsError());
    }
    // Get all the files from project folder
    let vec = read_files(files_path.as_ref())?;
    // Read covdir json and obtain all coverage information
    let file = fs::read_to_string(json_path)?;
    let covs = read_json_covdir(
        file,
        files_path
            .as_ref()
            .to_str()
            .ok_or(Error::PathConversionError())?,
    )?;
    let mut handlers = vec![];
    // Create a new Config all needed mutexes
    let cfg = Config::new();
    let (sender, receiver) = unbounded();
    let (sender_composer, receiver_composer) = unbounded();
    // Chunks the files vector
    let chunks = chunk_vector(vec, n_threads);
    debug!("Files divided in {} chunks", chunks.len());
    debug!("Launching all {} threads", n_threads);
    // Launch composer thread
    let composer =
        { thread::spawn(move || -> Result<JobComposer> { composer(receiver_composer) }) };
    // Launch n_threads consumer threads
    for _ in 0..n_threads {
        let r = receiver.clone();
        let s = sender_composer.clone();
        let config = cfg.clone();
        let h = thread::spawn(move || -> Result<()> { consumer_covdir(r, s, &config) });
        handlers.push(h);
    }
    let prefix = files_path
        .as_ref()
        .to_str()
        .ok_or(Error::PathConversionError())?
        .to_string()
        .len();
    chunks.iter().try_for_each(|chunk| {
        let job = JobItemCovDir::new(
            chunk.to_vec(),
            covs.clone(),
            metric,
            prefix,
            thresholds.to_vec(),
        );
        debug!("Sending job: {:?}", job);
        if let Err(_e) = sender.send(Some(job)) {
            return Err(Error::SenderError());
        }
        Ok(())
    })?;
    debug!("Poisoning threads...");
    // Stops all jobs by poisoning
    handlers.iter().try_for_each(|_| {
        if let Err(_e) = sender.send(None) {
            return Err(Error::SenderError());
        }
        Ok(())
    })?;
    debug!("Waiting for threads to finish...");
    // Wait the termination of all consumers
    for handle in handlers {
        handle.join()??;
    }
    if let Err(_e) = sender_composer.send(None) {
        return Err(Error::SenderError());
    }
    let mut files_ignored = cfg.files_ignored.lock()?;
    let mut res = cfg.res.lock()?;
    let project_coverage = covs
        .get(&("PROJECT_ROOT".to_string()))
        .ok_or(Error::HashMapError())?
        .coverage;
    // Get final  metrics for all the project
    let composer_output = composer.join()??;
    let project_metric = FileMetrics::new(
        get_project_metrics(composer_output, Some(project_coverage))?,
        "PROJECT".into(),
        "-".into(),
    );
    files_ignored.sort();
    res.sort_by(|a, b| a.file.cmp(&b.file));
    // Get AVG MIN MAX and complex files
    let mut complex_files = res
        .iter()
        .filter(|m| m.metrics.is_complex)
        .cloned()
        .collect::<Vec<FileMetrics>>();
    complex_files.sort_by(|a, b| match sort_by {
        Sort::WccPlain => b.metrics.wcc_plain.total_cmp(&a.metrics.wcc_plain),
        Sort::WccQuantized => b.metrics.wcc_quantized.total_cmp(&a.metrics.wcc_quantized),
        Sort::Crap => b.metrics.crap.total_cmp(&a.metrics.crap),
        Sort::Skunk => b.metrics.skunk.total_cmp(&a.metrics.skunk),
    });
    let m = res
        .iter()
        .map(|metric| metric.metrics)
        .collect::<Vec<Metrics>>();
    let (avg, max, min) = get_cumulative_values(&m);
    res.push(project_metric);
    res.push(FileMetrics::avg(avg));
    res.push(FileMetrics::max(max));
    res.push(FileMetrics::min(min));
    Ok((
        (*res).clone(),
        (*files_ignored).clone(),
        complex_files,
        project_coverage,
    ))
}

#[cfg(test)]
mod tests {

    use super::*;
    use crate::utility::compare_float;

    const JSON: &str = "./data/seahorse/seahorse.json";
    const COVDIR: &str = "./data/seahorse/covdir.json";
    const PROJECT: &str = "./data/seahorse/";
    const IGNORED: &str = "./data/seahorse/src/action.rs";

    #[test]
    fn test_metrics_coveralls_cyclomatic() {
        let json = Path::new(JSON);
        let project = Path::new(PROJECT);
        let ignored = Path::new(IGNORED);
        let (metrics, files_ignored, _, _) = get_metrics_concurrent(
            project,
            json,
            Complexity::Cyclomatic,
            8,
            &[30., 1.5, 35., 30.],
            Sort::WccPlain,
        )
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
        let json = Path::new(JSON);
        let project = Path::new(PROJECT);
        let ignored = Path::new(IGNORED);
        let (metrics, files_ignored, _, _) = get_metrics_concurrent(
            project,
            json,
            Complexity::Cognitive,
            8,
            &[30., 1.5, 35., 30.],
            Sort::WccPlain,
        )
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
        let covdir = Path::new(COVDIR);
        let project = Path::new(PROJECT);
        let ignored = Path::new(IGNORED);
        let (metrics, files_ignored, _, _) = get_metrics_concurrent_covdir(
            project,
            covdir,
            Complexity::Cyclomatic,
            8,
            &[30., 1.5, 35., 30.],
            Sort::WccPlain,
        )
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
        let covdir = Path::new(COVDIR);
        let project = Path::new(PROJECT);
        let ignored = Path::new(IGNORED);
        let (metrics, files_ignored, _, _) = get_metrics_concurrent_covdir(
            project,
            covdir,
            Complexity::Cognitive,
            8,
            &[30., 1.5, 35., 30.],
            Sort::WccPlain,
        )
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
