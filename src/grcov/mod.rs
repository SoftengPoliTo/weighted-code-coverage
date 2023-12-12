pub(crate) mod covdir;
pub(crate) mod coveralls;

use std::path::Path;

use crate::error::{Error, Result};

fn get_file_path<A: AsRef<Path>>(project_path: A, file_relative_path: &str) -> Result<String> {
    let mut splitted_file_relative_path = file_relative_path.split('/').collect::<Vec<&str>>();
    let mut splitted_project_path = project_path
        .as_ref()
        .as_os_str()
        .to_str()
        .ok_or(Error::PathConversion)?
        .split('/')
        .collect::<Vec<&str>>();
    splitted_project_path.pop();

    while let Some(s) = splitted_file_relative_path.pop() {
        if let Some(last) = splitted_project_path.last() {
            if *last == s {
                splitted_project_path.pop();
            }
        }
    }
    let file_path_prefix = splitted_project_path.join("/") + "/";

    Ok(Path::new(&file_path_prefix)
        .join(file_relative_path)
        .display()
        .to_string()
        .replace('\\', "/"))
}
