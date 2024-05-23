use std::{env::temp_dir, path::Path};

use insta::sorted_redaction;
use weighted_code_coverage::{GrcovFile, Mode, WccRunner};

const PROJECT_PATH: &str = "./tests/seahorse/";
const SNAPSHOTS_PATH: &str = "./snapshots/output/";
const COVERALLS_PATH: &str = "./tests/seahorse/coveralls.json";
const COVDIR_PATH: &str = "./tests/seahorse/covdir.json";
const JSON_OUTPUT: &str = "wcc.json";

#[test]
fn test_output_coveralls_files() {
    compare(
        GrcovFile::Coveralls(Path::new(COVERALLS_PATH)),
        Mode::Files,
        "output_coveralls_files",
    );
}

#[test]
fn test_output_covdir_files() {
    compare(
        GrcovFile::Covdir(Path::new(COVDIR_PATH)),
        Mode::Files,
        "output_covdir_files",
    );
}

#[test]
fn test_output_coveralls_functions() {
    compare(
        GrcovFile::Coveralls(Path::new(COVERALLS_PATH)),
        Mode::Functions,
        "output_coveralls_functions",
    );
}

#[test]
fn test_output_covdir_functions() {
    compare(
        GrcovFile::Covdir(Path::new(COVDIR_PATH)),
        Mode::Functions,
        "output_covdir_functions",
    );
}

fn compare(grcov_file: GrcovFile<&Path>, mode: Mode, snapshot_name: &str) {
    let output_dir = temp_dir();

    let output = WccRunner::new()
        .mode(mode)
        .json_path(&output_dir.join(JSON_OUTPUT))
        .html_path(&output_dir)
        .run(Path::new(PROJECT_PATH), grcov_file)
        .unwrap();

    insta::with_settings!({
        snapshot_path => Path::new(SNAPSHOTS_PATH),
        prepend_module_to_snapshot => false,
    },{
        insta::assert_yaml_snapshot!(snapshot_name, output, { ".files" => sorted_redaction(), ".ignored_files" => sorted_redaction(), ".files.*.functions" => sorted_redaction() });
    });
}
