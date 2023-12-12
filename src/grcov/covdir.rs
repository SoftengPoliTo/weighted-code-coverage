use serde_json::Value;
use std::{collections::HashMap, path::Path};

use super::get_file_path;
use crate::error::*;

#[derive(Debug)]
pub(crate) struct CovdirSourceFile {
    pub(crate) coverage: Vec<i32>,
    pub(crate) coverage_percent: f64,
}

#[derive(Debug)]
pub(crate) struct Covdir {
    pub(crate) source_files: HashMap<String, CovdirSourceFile>,
    pub(crate) total_coverage: f64,
}

impl Covdir {
    pub(crate) fn new<A: AsRef<Path>>(json: String, project_path: A) -> Result<Covdir> {
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
        source_files: &mut HashMap<String, CovdirSourceFile>,
        project_path: A,
    ) -> Result<()> {
        let mut stack = Vec::<(&Value, Vec<String>)>::new();
        stack.push((covdir_value, Vec::new()));

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

    // Handle the case where the json `&Value` popped from the stack is a directory
    fn handle_directory_value<'a>(
        directory_value: &'a Value,
        parent_directory: &Value,
        mut current_path: Vec<String>,
        stack: &mut Vec<(&'a Value, Vec<String>)>,
    ) -> Result<()> {
        if let Some(object) = directory_value.as_object() {
            let current_directory = get_directory(parent_directory)?;
            if !current_directory.is_empty() {
                current_path.push(current_directory);
            }
            for (_, child_object) in object {
                stack.push((child_object, current_path.clone()));
            }
        }

        Ok(())
    }

    // Handle the case where the json `&Value` popped from the stack is a source file
    fn handle_file_value<A: AsRef<Path>>(
        file_value: &Value,
        mut current_path: Vec<String>,
        project_path: A,
        source_files: &mut HashMap<String, CovdirSourceFile>,
    ) -> Result<()> {
        if let Some(name) = file_value.get("name").and_then(|n| n.as_str()) {
            if let (Some(json_coverage), Some(coverage_percent)) = (
                file_value.get("coverage").and_then(|c| c.as_array()),
                file_value.get("coveragePercent").and_then(|cp| cp.as_f64()),
            ) {
                current_path.push(name.to_string());
                let file_path = get_file_path(project_path, &current_path.join("/"))?;
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
fn get_directory(covdir_json: &Value) -> Result<String> {
    Ok(covdir_json
        .get("name")
        .ok_or(Error::Conversion)?
        .as_str()
        .ok_or(Error::Conversion)?
        .chars()
        .filter(|c| !c.is_whitespace())
        .collect::<String>())
}

#[inline]
fn parse_json_coverage(json_coverage: &[Value]) -> Vec<i32> {
    json_coverage
        .iter()
        .filter_map(|c| c.as_i64())
        .map(|v| v as i32)
        .collect::<Vec<i32>>()
}

#[inline]
fn get_total_coverage(covdir_json: &Value) -> Result<f64> {
    covdir_json
        .get("coveragePercent")
        .and_then(|cp| cp.as_f64())
        .ok_or(Error::Conversion)
}
