use std::{
    fs,
    path::{Path, PathBuf},
};

use serde_json::Value;
use weighted_code_coverage::{
    Complexity, GrcovFormat, Mode, OutputFormat, Sort, Thresholds, WccRunner,
};

const PROJECT_PATH: &str = "./tests/seahorse/";
const SNAPSHOTS_PATH: &str = "./snapshots/output/";
const COVERALLS_PATH: &str = "./tests/seahorse/coveralls.json";
const COVDIR_PATH: &str = "./tests/seahorse/covdir.json";

#[test]
fn test_output_cyclomatic_coveralls_files() {
    let output_path = "output_cyclomatic_coveralls_files.json";

    compare(
        Complexity::Cyclomatic,
        GrcovFormat::Coveralls(PathBuf::from(COVERALLS_PATH)),
        Mode::Files,
        output_path,
        output_path,
    );
}

#[test]
fn test_output_cyclomatic_covdir_files() {
    let output_path = "output_cyclomatic_covdir_files.json";

    compare(
        Complexity::Cyclomatic,
        GrcovFormat::Covdir(PathBuf::from(COVDIR_PATH)),
        Mode::Files,
        output_path,
        output_path,
    );
}

#[test]
fn test_output_cyclomatic_coveralls_functions() {
    let output_path = "output_cyclomatic_coveralls_functions.json";

    compare(
        Complexity::Cyclomatic,
        GrcovFormat::Coveralls(PathBuf::from(COVERALLS_PATH)),
        Mode::Functions,
        output_path,
        output_path,
    );
}

#[test]
fn test_output_cyclomatic_covdir_functions() {
    let output_path = "output_cyclomatic_covdir_functions.json";

    compare(
        Complexity::Cyclomatic,
        GrcovFormat::Covdir(PathBuf::from(COVDIR_PATH)),
        Mode::Functions,
        output_path,
        output_path,
    );
}

#[test]
fn test_output_cognitive_coveralls_files() {
    let output_path = "output_cognitive_coveralls_files.json";

    compare(
        Complexity::Cognitive,
        GrcovFormat::Coveralls(PathBuf::from(COVERALLS_PATH)),
        Mode::Files,
        output_path,
        output_path,
    );
}

#[test]
fn test_output_cognitive_covdir_files() {
    let output_path = "output_cognitive_covdir_files.json";

    compare(
        Complexity::Cognitive,
        GrcovFormat::Covdir(PathBuf::from(COVDIR_PATH)),
        Mode::Files,
        output_path,
        output_path,
    );
}

#[test]
fn test_output_cognitive_coveralls_functions() {
    let output_path = "output_cognitive_coveralls_functions.json";

    compare(
        Complexity::Cognitive,
        GrcovFormat::Coveralls(PathBuf::from(COVERALLS_PATH)),
        Mode::Functions,
        output_path,
        output_path,
    );
}

#[test]
fn test_output_cognitive_covdir_functions() {
    let output_path = "output_cognitive_covdir_functions.json";

    compare(
        Complexity::Cognitive,
        GrcovFormat::Covdir(PathBuf::from(COVDIR_PATH)),
        Mode::Functions,
        output_path,
        output_path,
    );
}

fn compare<P: AsRef<Path>>(
    complexity: Complexity,
    json_format: GrcovFormat,
    mode: Mode,
    output_path: P,
    snapshot_name: &str,
) {
    let output_path = std::env::temp_dir().join(output_path);

    WccRunner::new()
        .complexity((complexity, Thresholds(vec![30., 1.5, 35., 30.])))
        .n_threads(7)
        .grcov_format(json_format)
        .mode(mode)
        .sort_by(Sort::WccPlain)
        .output_format(OutputFormat::Json)
        .output_path(output_path.clone())
        .run(PROJECT_PATH)
        .unwrap();

    let json_string = fs::read_to_string(output_path).unwrap();
    let json_output: Value = serde_json::from_str(&json_string).unwrap();

    insta::with_settings!({
        snapshot_path => Path::new(SNAPSHOTS_PATH),
        prepend_module_to_snapshot => false
    },{
        insta::assert_json_snapshot!(snapshot_name, json_output);
    });
}
