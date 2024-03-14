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
    pub(crate) coverage_percent: f64,
}

#[derive(Debug, Serialize)]
pub(crate) struct Covdir {
    pub(crate) source_files: HashMap<PathBuf, CovdirSourceFile>,
    pub(crate) total_coverage: f64,
}

impl Covdir {
    pub(crate) fn new<A: AsRef<Path>, B: AsRef<Path>>(
        json_path: A,
        project_path: B,
    ) -> Result<Covdir> {
        let json = fs::read_to_string(json_path)?;
        let covdir_value: Value = serde_json::from_str(&json)?;
        let mut source_files = HashMap::new();
        Self::get_source_files(&covdir_value, &mut source_files, project_path)?;
        let total_coverage = get_total_coverage(&covdir_value)?;

        Ok(Covdir {
            source_files,
            total_coverage,
        })
    }

    // Finds all `CovdirSourceFile` and fills `source_files`
    // using the path of the files as the key.
    //
    // This function does an in-depth search,
    // exploring all directories and subdirectories.
    fn get_source_files<A: AsRef<Path>>(
        covdir_value: &Value,
        source_files: &mut HashMap<PathBuf, CovdirSourceFile>,
        project_path: A,
    ) -> Result<()> {
        let mut stack = Vec::<(&Value, PathBuf)>::new();
        stack.push((covdir_value, PathBuf::new()));

        while let Some((current_value, current_path)) = stack.pop() {
            if let Some(directory_value) = current_value.get("children") {
                Self::handle_directory_value(
                    directory_value,
                    current_value,
                    current_path,
                    &mut stack,
                )?;
            } else {
                Self::handle_file_value(current_value, current_path, &project_path, source_files)?;
            }
        }

        Ok(())
    }

    // Handle the case where the json `&Value` popped from the stack is a directory.
    fn handle_directory_value<'a>(
        directory_value: &'a Value,
        parent_directory: &Value,
        mut current_path: PathBuf,
        stack: &mut Vec<(&'a Value, PathBuf)>,
    ) -> Result<()> {
        if let Some(object) = directory_value.as_object() {
            let current_directory = get_directory(parent_directory)?;
            current_path.push(current_directory);
            for (_, child_object) in object {
                stack.push((child_object, current_path.clone()));
            }
        }

        Ok(())
    }

    // Handle the case where the json `&Value` popped from the stack is a source file.
    fn handle_file_value<A: AsRef<Path>>(
        file_value: &Value,
        mut current_path: PathBuf,
        project_path: A,
        source_files: &mut HashMap<PathBuf, CovdirSourceFile>,
    ) -> Result<()> {
        if let Some(name) = file_value.get("name").and_then(|n| n.as_str()) {
            if let (Some(json_coverage), Some(coverage_percent)) = (
                file_value.get("coverage").and_then(|c| c.as_array()),
                file_value.get("coveragePercent").and_then(|cp| cp.as_f64()),
            ) {
                current_path.push(name.to_string());
                let file_path = get_file_path(project_path, current_path);
                let coverage = parse_json_coverage(json_coverage);
                let source_file = CovdirSourceFile {
                    coverage,
                    coverage_percent,
                };

                source_files.insert(file_path, source_file);
            }
        }

        Ok(())
    }
}

#[inline]
fn get_directory(covdir_json: &Value) -> Result<PathBuf> {
    Ok(PathBuf::from(
        covdir_json
            .get("name")
            .ok_or(Error::Conversion)?
            .as_str()
            .ok_or(Error::Conversion)?,
    ))
}

#[inline]
fn get_file_path<A: AsRef<Path>, B: AsRef<Path>>(project_path: A, file_relative_path: B) -> PathBuf {
    let file_path = project_path.as_ref().to_path_buf().join(file_relative_path);

    PathBuf::from(file_path.to_string_lossy().replace('\\', "/"))
}

#[inline]
fn parse_json_coverage(json_coverage: &[Value]) -> Vec<Option<i32>> {
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
        let covdir = Covdir::new(COVDIR_PATH, Path::new("./project/path/")).unwrap();

        insta::with_settings!({sort_maps => true}, {
            insta::assert_yaml_snapshot!(covdir, @r###"
            ---
            source_files:
              "./project/path/src/app.rs":
                coverage:
                  - -1
                  - -1
                coverage_percent: 86.62
              "./project/path/src/inner_module/inner_module_file.rs":
                coverage:
                  - -1
                  - -1
                coverage_percent: 0
              "./project/path/src/inner_module/mod.rs":
                coverage:
                  - 0
                  - -1
                coverage_percent: 0
              "./project/path/src/lib.rs":
                coverage:
                  - 2
                coverage_percent: 100
            total_coverage: 77.21
            "###);
        });
    }
}
