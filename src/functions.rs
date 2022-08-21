use std::collections::HashMap;
use std::fmt;
use std::fs;
use std::path::*;
use std::sync::{Arc, Mutex};
use std::thread;

use crossbeam::channel::{unbounded, Receiver};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tracing::debug;

use crate::error::*;
use crate::files::*;
use crate::utility::*;

/// Struct with all the metrics computed for the root
#[derive(Clone, Default, Debug, Serialize, Deserialize, PartialEq)]
#[allow(dead_code)]
pub struct RootMetrics {
    pub metrics: Metrics,
    pub file_name: String,
    pub file_path: String,
    pub start_line: usize,
    pub end_line: usize,
    pub functions: Vec<FunctionMetrics>,
}
impl RootMetrics {
    pub fn new(
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

    pub fn avg(m: Metrics) -> Self {
        Self {
            metrics: m,
            file_name: "AVG".into(),
            file_path: "-".into(),
            start_line: 0,
            end_line: 0,
            functions: Vec::<FunctionMetrics>::new(),
        }
    }

    pub fn min(m: Metrics) -> Self {
        Self {
            metrics: m,
            file_name: "MIN".into(),
            file_path: "-".into(),
            start_line: 0,
            end_line: 0,
            functions: Vec::<FunctionMetrics>::new(),
        }
    }

    pub fn max(m: Metrics) -> Self {
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

/// Struct with all the metrics computed for a single function
#[derive(Clone, Default, Debug, Serialize, Deserialize, PartialEq)]
#[allow(dead_code)]
pub struct FunctionMetrics {
    pub metrics: Metrics,
    pub function_name: String,
    pub function_path: String,
    pub start_line: usize,
    pub end_line: usize,
}
impl FunctionMetrics {
    pub fn new(
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

type Output = (Vec<RootMetrics>, Vec<String>, Vec<FunctionMetrics>, f64);

// job received by the consumer threads
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

// Configuration shared by all threads with all the data that must be returned
#[derive(Clone, Default, Debug)]
pub struct FunctionConfig {
    pub(crate) res: Arc<Mutex<Vec<RootMetrics>>>,
    pub(crate) files_ignored: Arc<Mutex<Vec<String>>>,
}

impl FunctionConfig {
    fn new() -> Self {
        Self {
            res: Arc::new(Mutex::new(Vec::<RootMetrics>::new())),
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
fn consumer(
    receiver: JobReceiver,
    sender_composer: ComposerSender,
    cfg: &FunctionConfig,
) -> Result<()> {
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
            let spaces = get_spaces(&root)?;
            let ploc = root.metrics.loc.ploc();
            let comp = match metric {
                Complexity::Cyclomatic => root.metrics.cyclomatic.cyclomatic_sum(),
                Complexity::Cognitive => root.metrics.cognitive.cognitive_sum(),
            };
            let mut functions = Vec::<FunctionMetrics>::new();
            spaces.iter().try_for_each(|el| -> Result<()> {
                let space = el.0;
                let function_path = el.1.to_string();
                let (m, _): (Metrics, (f64, f64)) =
                    Tree::get_metrics_from_space(space, &arr, metric, None, &thresholds)?;
                let function_name = format!(
                    "{} ({}, {})",
                    space.name.as_ref().ok_or(Error::PathConversionError())?,
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
            let (m, (sp_sum, sq_sum)): (Metrics, (f64, f64)) =
                Tree::get_metrics_from_space(&root, &arr, metric, None, &thresholds)?;
            let file_path = file.clone().split_off(prefix);
            // Upgrade all the global variables and add metrics to the result and complex_files
            let mut res = res.lock()?;
            composer_output.covered_lines += covered_lines;
            composer_output.total_lines += tot_lines;
            composer_output.ploc_sum += ploc;
            composer_output.wcc_plain_sum += sp_sum;
            composer_output.wcc_quantized_sum += sq_sum;
            composer_output.comp_sum += comp;
            res.push(RootMetrics::new(
                m,
                file_name,
                file_path,
                root.start_line,
                root.end_line,
                functions,
            ));
        }
    }
    if let Err(_e) = sender_composer.send(Some(composer_output)) {
        println!("{}", _e);
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
pub fn get_functions_metrics_concurrent<A: AsRef<Path>, B: AsRef<Path>>(
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
    let cfg = FunctionConfig::new();
    let (sender, receiver) = unbounded();
    let (sender_composer, receiver_composer) = unbounded();
    // Chunks the files vector
    let chunks = chunk_vector(vec, n_threads);
    debug!("Files divided in {} chunks", chunks.len());
    debug!("Launching all {} threads", n_threads);
    let composer =
        { thread::spawn(move || -> Result<JobComposer> { composer(receiver_composer) }) };
    for _ in 0..n_threads {
        let r = receiver.clone();
        let s = sender_composer.clone();
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
    let project_metric = RootMetrics::new(
        get_project_metrics(composer_output, None)?,
        "PROJECT".into(),
        "-".into(),
        0,
        0,
        Vec::<FunctionMetrics>::new(),
    );
    let project_coverage = project_metric.metrics.coverage;
    files_ignored.sort();
    res.sort_by(|a, b| a.file_name.cmp(&b.file_name));
    // Get AVG MIN MAX and complex files
    let mut complex_functions = res
        .iter()
        .flat_map(|m| m.functions.clone())
        .filter(|m| m.metrics.is_complex)
        .collect::<Vec<FunctionMetrics>>();
    complex_functions.sort_by(|a, b| match sort_by {
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
    res.push(RootMetrics::avg(avg));
    res.push(RootMetrics::max(max));
    res.push(RootMetrics::min(min));
    Ok((
        (*res).clone(),
        (*files_ignored).clone(),
        complex_functions,
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

// Consumer function run by ead independent thread
fn consumer_covdir(
    receiver: JobReceiverCovDir,
    sender_composer: ComposerSender,
    cfg: &FunctionConfig,
) -> Result<()> {
    // Get all shared data
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
        // For each file in the chunk received
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
            let spaces = get_spaces(&root)?;
            let ploc = root.metrics.loc.ploc();
            let comp = match metric {
                Complexity::Cyclomatic => root.metrics.cyclomatic.cyclomatic_sum(),
                Complexity::Cognitive => root.metrics.cognitive.cognitive_sum(),
            };
            let mut functions = Vec::<FunctionMetrics>::new();
            spaces.iter().try_for_each(|el| -> Result<()> {
                let space = el.0;
                let function_path = el.1.to_string();
                let function_name = format!(
                    "{} ({}, {})",
                    space.name.as_ref().ok_or(Error::ConversionError())?,
                    space.start_line,
                    space.end_line
                );
                let (m, _): (Metrics, (f64, f64)) =
                    Tree::get_metrics_from_space(space, arr, metric, coverage, &thresholds)?;
                functions.push(FunctionMetrics::new(
                    m,
                    function_name,
                    function_path,
                    space.start_line,
                    space.end_line,
                ));
                Ok(())
            })?;
            let file_path = file.clone().split_off(prefix);
            let (m, (sp_sum, sq_sum)): (Metrics, (f64, f64)) =
                Tree::get_metrics_from_space(&root, arr, metric, coverage, &thresholds)?;
            // Upgrade all the global variables and add metrics to the result and complex_files
            let mut res = res.lock()?;
            composer_output.ploc_sum += ploc;
            composer_output.wcc_plain_sum += sp_sum;
            composer_output.wcc_quantized_sum += sq_sum;
            composer_output.comp_sum += comp;
            res.push(RootMetrics::new(
                m,
                file_name,
                file_path,
                root.start_line,
                root.end_line,
                functions,
            ));
        }
    }
    if let Err(_e) = sender_composer.send(Some(composer_output)) {
        println!("{}", _e);
        return Err(Error::SenderError());
    }
    Ok(())
}

pub fn get_functions_metrics_concurrent_covdir<A: AsRef<Path>, B: AsRef<Path>>(
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
    let covs = read_json_covdir(
        file,
        files_path
            .as_ref()
            .to_str()
            .ok_or(Error::PathConversionError())?,
    )?;
    let mut handlers = vec![];
    // Create a new config with  all needed mutexes
    let cfg = FunctionConfig::new();
    let (sender, receiver) = unbounded();
    let (sender_composer, receiver_composer) = unbounded();
    // Chunks the files vector
    let chunks = chunk_vector(vec, n_threads);
    debug!("Files divided in {} chunks", chunks.len());
    debug!("Launching all {} threads", n_threads);
    let composer =
        { thread::spawn(move || -> Result<JobComposer> { composer(receiver_composer) }) };
    for _ in 0..n_threads {
        let r = receiver.clone();
        let s = sender_composer.clone();
        let config = cfg.clone();
        // Launch n_threads consume threads
        let h = thread::spawn(move || -> Result<()> { consumer_covdir(r, s, &config) });
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
    let project_coverage = covs
        .get(&("PROJECT_ROOT".to_string()))
        .ok_or(Error::HashMapError())?
        .coverage;
    let composer_output = composer.join()??;
    let project_metric = RootMetrics::new(
        get_project_metrics(composer_output, Some(project_coverage))?,
        "PROJECT".into(),
        "-".into(),
        0,
        0,
        Vec::<FunctionMetrics>::new(),
    );
    files_ignored.sort();
    res.sort_by(|a, b| a.file_name.cmp(&b.file_name));
    // Get AVG MIN MAX and complex files
    let mut complex_functions = res
        .iter()
        .flat_map(|m| m.functions.clone())
        .filter(|m| m.metrics.is_complex)
        .collect::<Vec<FunctionMetrics>>();
    complex_functions.sort_by(|a, b| match sort_by {
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
    res.push(RootMetrics::avg(avg));
    res.push(RootMetrics::max(max));
    res.push(RootMetrics::min(min));
    Ok((
        (*res).clone(),
        (*files_ignored).clone(),
        complex_functions,
        f64::round(project_coverage * 100.) / 100.,
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
        let (metrics, files_ignored, _, _) = get_functions_metrics_concurrent(
            project,
            json,
            Complexity::Cyclomatic,
            8,
            &[30., 1.5, 35., 30.],
            Sort::WccPlain,
        )
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
        let json = Path::new(JSON);
        let project = Path::new(PROJECT);
        let ignored = Path::new(IGNORED);
        let (metrics, files_ignored, _, _) = get_functions_metrics_concurrent(
            project,
            json,
            Complexity::Cognitive,
            8,
            &[30., 1.5, 35., 30.],
            Sort::WccPlain,
        )
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
        let covdir = Path::new(COVDIR);
        let project = Path::new(PROJECT);
        let ignored = Path::new(IGNORED);
        let (metrics, files_ignored, _, _) = get_functions_metrics_concurrent_covdir(
            project,
            covdir,
            Complexity::Cyclomatic,
            8,
            &[30., 1.5, 35., 30.],
            Sort::WccPlain,
        )
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
        let covdir = Path::new(COVDIR);
        let project = Path::new(PROJECT);
        let ignored = Path::new(IGNORED);
        let (metrics, files_ignored, _, _) = get_functions_metrics_concurrent_covdir(
            project,
            covdir,
            Complexity::Cognitive,
            8,
            &[30., 1.5, 35., 30.],
            Sort::WccPlain,
        )
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
