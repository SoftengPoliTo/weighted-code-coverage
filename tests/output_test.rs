use std::path::{Path, PathBuf};

use weighted_code_coverage::{Complexity, ComplexityType, GrcovFormat, Mode, WccRunner};

const PROJECT_PATH: &str = "./tests/seahorse/";
const SNAPSHOTS_PATH: &str = "./snapshots/output/";
const COVERALLS_PATH: &str = "./tests/seahorse/coveralls.json";
const COVDIR_PATH: &str = "./tests/seahorse/covdir.json";

#[test]
fn test_output_cyclomatic_coveralls_files() {
    compare(
        ComplexityType::Cyclomatic,
        GrcovFormat::Coveralls(PathBuf::from(COVERALLS_PATH)),
        Mode::Files,
        "output_cyclomatic_coveralls_files",
    );
}

#[test]
fn test_output_cyclomatic_covdir_files() {
    compare(
        ComplexityType::Cyclomatic,
        GrcovFormat::Covdir(PathBuf::from(COVDIR_PATH)),
        Mode::Files,
        "output_cyclomatic_covdir_files",
    );
}

#[test]
fn test_output_cyclomatic_coveralls_functions() {
    compare(
        ComplexityType::Cyclomatic,
        GrcovFormat::Coveralls(PathBuf::from(COVERALLS_PATH)),
        Mode::Functions,
        "output_cyclomatic_coveralls_functions",
    );
}

#[test]
fn test_output_cyclomatic_covdir_functions() {
    compare(
        ComplexityType::Cyclomatic,
        GrcovFormat::Covdir(PathBuf::from(COVDIR_PATH)),
        Mode::Functions,
        "output_cyclomatic_covdir_functions",
    );
}

#[test]
fn test_output_cognitive_coveralls_files() {
    compare(
        ComplexityType::Cognitive,
        GrcovFormat::Coveralls(PathBuf::from(COVERALLS_PATH)),
        Mode::Files,
        "output_cognitive_coveralls_files",
    );
}

#[test]
fn test_output_cognitive_covdir_files() {
    compare(
        ComplexityType::Cognitive,
        GrcovFormat::Covdir(PathBuf::from(COVDIR_PATH)),
        Mode::Files,
        "output_cognitive_covdir_files",
    );
}

#[test]
fn test_output_cognitive_coveralls_functions() {
    compare(
        ComplexityType::Cognitive,
        GrcovFormat::Coveralls(PathBuf::from(COVERALLS_PATH)),
        Mode::Functions,
        "output_cognitive_coveralls_functions",
    );
}

#[test]
fn test_output_cognitive_covdir_functions() {
    compare(
        ComplexityType::Cognitive,
        GrcovFormat::Covdir(PathBuf::from(COVDIR_PATH)),
        Mode::Functions,
        "output_cognitive_covdir_functions",
    );
}

fn compare<A: AsRef<Path> + Default>(
    complexity_type: ComplexityType,
    json_format: GrcovFormat<A>,
    mode: Mode,
    snapshot_name: &str,
) {
    let output = WccRunner::<A, A>::new()
        .complexity(Complexity {
            complexity_type,
            ..Complexity::default()
        })
        .grcov_format(json_format)
        .mode(mode)
        .run(PROJECT_PATH)
        .unwrap();

    insta::with_settings!({
        snapshot_path => Path::new(SNAPSHOTS_PATH),
        prepend_module_to_snapshot => false
    },{
        insta::assert_snapshot!(snapshot_name, serde_json::to_string_pretty(&output).unwrap());
    });
}
