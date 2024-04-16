use serde::Serialize;
use serde_json::Value;
use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
};

use crate::error::*;

#[derive(Debug, Serialize)]
pub(crate) struct CovdirSourceFile {
    pub(crate) coverage: Vec<Option<i32>>,
    coverage_percent: f64,
}

#[derive(Debug, Serialize)]
pub(crate) struct Covdir {
    pub(crate) source_files: HashMap<PathBuf, CovdirSourceFile>,
    total_coverage: f64,
}

impl Covdir {
    pub(crate) fn new(json_path: &Path, project_path: &Path) -> Result<Covdir> {
        let json = fs::read_to_string(json_path)?;
        let covdir_value: Value = serde_json::from_str(&json)?;
        let mut source_files = HashMap::new();
        get_source_files(&covdir_value, &mut source_files, project_path);
        let total_coverage = get_total_coverage(&covdir_value)?;

        Ok(Covdir {
            source_files,
            total_coverage,
        })
    }
}

// Finds all `CovdirSourceFile` and fills `source_files`
// using the path of the files as the key.
//
// This function does an in-depth search,
// exploring all directories and subdirectories.
fn get_source_files(
    covdir_value: &Value,
    source_files: &mut HashMap<PathBuf, CovdirSourceFile>,
    project_path: &Path,
) {
    let mut stack = Vec::<(&Value, PathBuf)>::new();
    stack.push((covdir_value, PathBuf::new()));

    while let Some((current_value, current_path)) = stack.pop() {
        if let Some(directory_value) = current_value.get("children") {
            handle_directory_value(directory_value, current_value, current_path, &mut stack);
        } else {
            handle_file_value(current_value, current_path, project_path, source_files);
        }
    }
}

// Handle the case where the json `&Value` popped from the stack is a directory.
fn handle_directory_value<'a>(
    directory_value: &'a Value,
    parent_directory: &Value,
    mut current_path: PathBuf,
    stack: &mut Vec<(&'a Value, PathBuf)>,
) {
    if let (Some(object), Some(current_directory)) =
        (directory_value.as_object(), get_directory(parent_directory))
    {
        current_path.push(current_directory);
        object
            .iter()
            .for_each(|(_, child_object)| stack.push((child_object, current_path.clone())));
    }
}

// Handle the case where the json `&Value` popped from the stack is a source file.
fn handle_file_value(
    file_value: &Value,
    mut current_path: PathBuf,
    project_path: &Path,
    source_files: &mut HashMap<PathBuf, CovdirSourceFile>,
) {
    if let Some(name) = file_value.get("name").and_then(|n| n.as_str()) {
        if let (Some(lines_coverage), Some(coverage_percentage)) = (
            file_value.get("coverage").and_then(|c| c.as_array()),
            file_value.get("coveragePercent").and_then(|cp| cp.as_f64()),
        ) {
            current_path.push(name);
            let file_path = get_file_path(project_path, &current_path);
            let coverage = parse_coverage(lines_coverage);
            let source_file = CovdirSourceFile {
                coverage,
                coverage_percent: coverage_percentage,
            };

            source_files.insert(file_path, source_file);
        }
    }
}

#[inline]
fn get_directory(covdir_json: &Value) -> Option<PathBuf> {
    covdir_json.get("name")?.as_str().map(PathBuf::from)
}

#[inline]
fn get_file_path(project_path: &Path, file_relative_path: &Path) -> PathBuf {
    let file_path = project_path.join(file_relative_path);

    PathBuf::from(file_path.to_string_lossy().replace('\\', "/"))
}

#[inline]
fn parse_coverage(json_coverage: &[Value]) -> Vec<Option<i32>> {
    // Coverage values are converted to `Option<i32>`
    // to be consistent with the coveralls format,
    // that uses `null` instead of -1 to represent SLOC lines,
    // which are comment or blank lines that don't require coverage.
    json_coverage
        .iter()
        .filter_map(|c| c.as_i64())
        .map(|v| if v == -1 { None } else { Some(v as i32) })
        .collect::<Vec<Option<i32>>>()
}

#[inline]
fn get_total_coverage(covdir_json: &Value) -> Result<f64> {
    covdir_json
        .get("coveragePercent")
        .and_then(|cp| cp.as_f64())
        .ok_or(Error::Conversion)
}

#[cfg(test)]
mod tests {

    use super::Covdir;
    use std::path::Path;

    const COVDIR_PATH: &str = "./tests/grcov_files/grcov_covdir.json";

    #[test]
    fn test_covdir() {
        let covdir = Covdir::new(Path::new(COVDIR_PATH), Path::new("project/test/path/")).unwrap();

        insta::with_settings!({sort_maps => true}, {
            insta::assert_yaml_snapshot!(covdir, @r###"
            ---
            source_files:
              project/test/path/src/app.rs:
                coverage:
                  - ~
                  - ~
                coverage_percent: 86.62
              project/test/path/src/inner_module/inner_module_file.rs:
                coverage:
                  - ~
                  - ~
                coverage_percent: 0
              project/test/path/src/inner_module/mod.rs:
                coverage:
                  - 0
                  - ~
                coverage_percent: 0
              project/test/path/src/lib.rs:
                coverage:
                  - 2
                coverage_percent: 100
            total_coverage: 77.21
            "###);
        });
    }
}
