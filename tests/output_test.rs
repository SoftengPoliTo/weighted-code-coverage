use std::path::{Path, PathBuf};

use weighted_code_coverage::{
    Complexity, GrcovFormat, Mode, OutputFormat, Sort, Thresholds, WccRunner,
};

const PROJECT_PATH: &str = "./tests/seahorse/";
const SNAPSHOTS_PATH: &str = "./snapshots/output/";
const COVERALLS_PATH: &str = "./tests/seahorse/coveralls.json";
const COVDIR_PATH: &str = "./tests/seahorse/covdir.json";

#[test]
fn test_output_cyclomatic_coveralls_files() {
    compare(
        Complexity::Cyclomatic,
        GrcovFormat::Coveralls(PathBuf::from(COVERALLS_PATH)),
        Mode::Files,
        "output_cyclomatic_coveralls_files",
    );
}

#[test]
fn test_output_cyclomatic_covdir_files() {
    compare(
        Complexity::Cyclomatic,
        GrcovFormat::Covdir(PathBuf::from(COVDIR_PATH)),
        Mode::Files,
        "output_cyclomatic_covdir_files",
    );
}

#[test]
fn test_output_cyclomatic_coveralls_functions() {
    compare(
        Complexity::Cyclomatic,
        GrcovFormat::Coveralls(PathBuf::from(COVERALLS_PATH)),
        Mode::Functions,
        "output_cyclomatic_coveralls_functions",
    );
}

#[test]
fn test_output_cyclomatic_covdir_functions() {
    compare(
        Complexity::Cyclomatic,
        GrcovFormat::Covdir(PathBuf::from(COVDIR_PATH)),
        Mode::Functions,
        "output_cyclomatic_covdir_functions",
    );
}

#[test]
fn test_output_cognitive_coveralls_files() {
    compare(
        Complexity::Cognitive,
        GrcovFormat::Coveralls(PathBuf::from(COVERALLS_PATH)),
        Mode::Files,
        "output_cognitive_coveralls_files",
    );
}

#[test]
fn test_output_cognitive_covdir_files() {
    compare(
        Complexity::Cognitive,
        GrcovFormat::Covdir(PathBuf::from(COVDIR_PATH)),
        Mode::Files,
        "output_cognitive_covdir_files",
    );
}

#[test]
fn test_output_cognitive_coveralls_functions() {
    compare(
        Complexity::Cognitive,
        GrcovFormat::Coveralls(PathBuf::from(COVERALLS_PATH)),
        Mode::Functions,
        "output_cognitive_coveralls_functions",
    );
}

#[test]
fn test_output_cognitive_covdir_functions() {
    compare(
        Complexity::Cognitive,
        GrcovFormat::Covdir(PathBuf::from(COVDIR_PATH)),
        Mode::Functions,
        "output_cognitive_covdir_functions",
    );
}

fn compare<A: AsRef<Path> + Default>(
    complexity: Complexity,
    json_format: GrcovFormat<A>,
    mode: Mode,
    snapshot_name: &str,
) {
    let output = WccRunner::<A, A>::new()
        .complexity((complexity, Thresholds(vec![30., 1.5, 35., 30.])))
        .n_threads(7)
        .grcov_format(json_format)
        .mode(mode)
        .sort_by(Sort::WccPlain)
        .output_format(OutputFormat::Json)
        .run(PROJECT_PATH)
        .unwrap();

    insta::with_settings!({
        snapshot_path => Path::new(SNAPSHOTS_PATH),
        prepend_module_to_snapshot => false
    },{
        insta::assert_snapshot!(snapshot_name, serde_json::to_string_pretty(&output).unwrap());
    });
}
