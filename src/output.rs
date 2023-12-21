use std::fs;
use std::fs::File;
use std::path::*;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use tera::Context;
use tera::Tera;
use tracing::debug;

use crate::concurrent::files::*;
use crate::concurrent::functions::*;
use crate::error::*;
use crate::Sort;

// Struct for JSON for files
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
struct JSONOutput {
    project_folder: String,
    number_of_files_ignored: usize,
    number_of_complex_files: usize,
    metrics: Vec<FileMetrics>,
    files_ignored: Vec<String>,
    complex_files: Vec<FileMetrics>,
    project_coverage: f64,
}

// Struct for JSON for functions
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
struct JSONOutputFunc {
    project_folder: String,
    number_of_files_ignored: usize,
    number_of_complex_functions: usize,
    files: Vec<RootMetrics>,
    files_ignored: Vec<String>,
    complex_functions: Vec<FunctionMetrics>,
    project_coverage: f64,
}

#[derive(Serialize)]
struct HTMLTemplateFile {
    project_folder: String,
    number_of_files_ignored: usize,
    number_of_complex_files: usize,
    metrics: Vec<FileMetrics>,
    files_ignored: Vec<String>,
    complex_files: Vec<FileMetrics>,
    project_coverage: f64,
    bulma_version: String,
    date: DateTime<Utc>,
}

#[derive(Serialize)]
struct HTMLTemplateFunction {
    project_folder: String,
    number_of_files_ignored: usize,
    number_of_complex_files: usize,
    metrics: Vec<RootMetrics>,
    files_ignored: Vec<String>,
    complex_functions: Vec<FunctionMetrics>,
    project_coverage: f64,
    bulma_version: String,
    date: DateTime<Utc>,
}

trait PrintResult<T> {
    fn print_result(result: &T, files_ignored: usize, complex_files: usize);
    fn print_json_to_file(
        result: &T,
        files_ignored: &[String],
        project_coverage: f64,
        json_path: &Path,
        project_folder: &Path,
        sort_by: Sort,
    ) -> Result<()>;
    fn print_html_to_file(
        result: &T,
        files_ignored: &[String],
        html: &Path,
        project_folder: &Path,
        project_coverage: f64,
        sort_by: Sort,
    ) -> Result<()>;
}
struct Text;

impl PrintResult<Vec<FileMetrics>> for Text {
    fn print_result(result: &Vec<FileMetrics>, files_ignored: usize, complex_files: usize) {
        println!(
            "{0: <20} | {1: <20} | {2: <20} | {3: <20} | {4: <20} | {5: <20} | {6: <30}",
            "FILE", "WCC PLAIN", "WCC QUANTIZED", "CRAP", "SKUNKSCORE", "IS_COMPLEX", "PATH"
        );
        result.iter().for_each(|m| {
            println!(
                "{0: <20} | {1: <20.3} | {2: <20.3} | {3: <20.3} | {4: <20.3} | {5: <20} | {6: <30}",
                m.file,
                m.metrics.wcc_plain,
                m.metrics.wcc_quantized,
                m.metrics.crap,
                m.metrics.skunk,
                m.metrics.is_complex,
                m.file_path
            );
        });
        println!("FILES IGNORED: {files_ignored}");
        println!("COMPLEX FILES: {complex_files}");
    }
    fn print_json_to_file(
        result: &Vec<FileMetrics>,
        files_ignored: &[String],
        project_coverage: f64,
        json_path: &Path,
        project_folder: &Path,
        sort_by: Sort,
    ) -> Result<()> {
        let mut complex_files = result
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
        let json = export_to_json(
            project_folder,
            result,
            files_ignored,
            &complex_files,
            project_coverage,
        );
        serde_json::to_writer(&File::create(json_path)?, &json)?;
        Ok(())
    }
    fn print_html_to_file(
        result: &Vec<FileMetrics>,
        files_ignored: &[String],
        html: &Path,
        project_folder: &Path,
        project_coverage: f64,
        sort_by: Sort,
    ) -> Result<()> {
        let tera = match Tera::new("src/templates/*.html") {
            Ok(t) => t,
            Err(e) => {
                println!("Parsing error(s): {e}");
                ::std::process::exit(1);
            }
        };
        let mut complex_files = result
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
        let template = HTMLTemplateFile {
            project_folder: project_folder.display().to_string(),
            number_of_files_ignored: files_ignored.len(),
            number_of_complex_files: complex_files.len(),
            metrics: result.to_vec(),
            files_ignored: files_ignored.to_vec(),
            complex_files,
            project_coverage,
            bulma_version: "0.9.1".to_string(),
            date: Utc::now(),
        };
        let output = tera.render("files.html", &Context::from_serialize(template)?)?;
        //let mut file = File::create("./output/index.html")?;
        fs::write(html, output)?;
        Ok(())
    }
}
impl PrintResult<Vec<RootMetrics>> for Text {
    fn print_result(result: &Vec<RootMetrics>, files_ignored: usize, complex_files: usize) {
        println!(
            "{0: <20} | {1: <20} | {2: <20} | {3: <20} | {4: <20} | {5: <20} | {6: <30}",
            "FUNCTION", "WCC PLAIN", "WCC QUANTIZED", "CRAP", "SKUNKSCORE", "IS_COMPLEX", "PATH"
        );
        result.iter().for_each(|m| {
            println!(
                "{0: <20} | {1: <20.3} | {2: <20.3} | {3: <20.3} | {4: <20.3} | {5: <20} | {6: <30}",
                m.file_name,
                m.metrics.wcc_plain,
                m.metrics.wcc_quantized,
                m.metrics.crap,
                m.metrics.skunk,
                m.metrics.is_complex,
                m.file_path
            );
            m.functions.iter().for_each(|f|{
                println!(
                    "{0: <20} | {1: <20.3} | {2: <20.3} | {3: <20.3} | {4: <20.3} | {5: <20} | {6: <30}",
                    f.function_name,
                    f.metrics.wcc_plain,
                    f.metrics.wcc_quantized,
                    f.metrics.crap,
                    f.metrics.skunk,
                    f.metrics.is_complex,
                    f.function_path
                );
            });
        });
        println!("FILES IGNORED: {files_ignored}");
        println!("COMPLEX FUNCTIONS: {complex_files}");
    }
    fn print_json_to_file(
        result: &Vec<RootMetrics>,
        files_ignored: &[String],
        project_coverage: f64,
        json_path: &Path,
        project_folder: &Path,
        sort_by: Sort,
    ) -> Result<()> {
        let mut complex_functions: Vec<FunctionMetrics> = result
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
        let json = export_to_json_function(
            project_folder,
            result,
            files_ignored,
            &complex_functions,
            project_coverage,
        );
        serde_json::to_writer(&File::create(json_path)?, &json)?;
        Ok(())
    }
    fn print_html_to_file(
        result: &Vec<RootMetrics>,
        files_ignored: &[String],
        html: &Path,
        project_folder: &Path,
        project_coverage: f64,
        sort_by: Sort,
    ) -> Result<()> {
        let tera = match Tera::new("src/templates/*.html") {
            Ok(t) => t,
            Err(e) => {
                println!("Parsing error(s): {e}");
                ::std::process::exit(1);
            }
        };
        let mut complex_functions: Vec<FunctionMetrics> = result
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
        let template = HTMLTemplateFunction {
            project_folder: project_folder.display().to_string(),
            number_of_files_ignored: files_ignored.len(),
            number_of_complex_files: complex_functions.len(),
            metrics: result.to_vec(),
            files_ignored: files_ignored.to_vec(),
            complex_functions,
            project_coverage,
            bulma_version: "0.9.1".to_string(),
            date: Utc::now(),
        };
        let output = tera.render("functions.html", &Context::from_serialize(template)?)?;
        //let mut file = File::create("./output/index.html")?;
        fs::write(html, output)?;
        Ok(())
    }
}

// Export all metrics to a json file
fn export_to_json(
    project_folder: &Path,
    metrics: &[FileMetrics],
    files_ignored: &[String],
    complex_files: &Vec<FileMetrics>,
    project_coverage: f64,
) -> JSONOutput {
    let number_of_files_ignored = files_ignored.len();
    let number_of_complex_files = complex_files.len();

    JSONOutput {
        project_folder: project_folder.display().to_string(),
        number_of_files_ignored,
        number_of_complex_files,
        metrics: metrics.to_vec(),
        files_ignored: files_ignored.to_vec(),
        complex_files: complex_files.to_vec(),
        project_coverage,
    }
}

// Export all metrics to a json file for functions mode
fn export_to_json_function(
    project_folder: &Path,
    metrics: &[RootMetrics],
    files_ignored: &[String],
    complex_functions: &Vec<FunctionMetrics>,
    project_coverage: f64,
) -> JSONOutputFunc {
    let number_of_files_ignored = files_ignored.len();
    let number_of_complex_functions = complex_functions.len();
    JSONOutputFunc {
        project_folder: project_folder.display().to_string(),
        number_of_files_ignored,
        number_of_complex_functions,
        files: metrics.to_vec(),
        files_ignored: files_ignored.to_vec(),
        complex_functions: complex_functions.to_vec(),
        project_coverage,
    }
}

// This Function get the folder of the repo to analyzed and the path to the json obtained using grcov
// It prints all the WCC, CRAP and SkunkScore values for all the files in the folders
// the output will be print as follows:
// FILE       | WCC PLAIN | WCC QUANTIZED | CRAP       | SKUNKSCORE | "IS_COMPLEX" | "PATH"
// if the a file is not found in the json that files will be skipped

pub(crate) fn get_metrics_output(
    metrics: &Vec<FileMetrics>,
    files_ignored: &Vec<String>,
    complex_files: &Vec<FileMetrics>,
) {
    Text::print_result(metrics, files_ignored.len(), complex_files.len());
}

// Prints the the given  metrics ,files ignored and complex files  in a json format
pub(crate) fn print_metrics_to_json<A: AsRef<Path>, B: AsRef<Path>>(
    metrics: &Vec<FileMetrics>,
    files_ignored: &[String],
    json_output: A,
    project_folder: B,
    project_coverage: f64,
    sort_by: Sort,
) -> Result<()> {
    debug!("Exporting to json...");
    Text::print_json_to_file(
        metrics,
        files_ignored,
        project_coverage,
        json_output.as_ref(),
        project_folder.as_ref(),
        sort_by,
    )
}

// Prints the the given  metrics ,files ignored and complex files  in a json format
pub(crate) fn print_metrics_to_html<A: AsRef<Path>, B: AsRef<Path>>(
    metrics: &Vec<FileMetrics>,
    files_ignored: &[String],
    html: A,
    project_folder: B,
    project_coverage: f64,
    sort_by: Sort,
) -> Result<()> {
    debug!("Exporting to HTML...");
    Text::print_html_to_file(
        metrics,
        files_ignored,
        html.as_ref(),
        project_folder.as_ref(),
        project_coverage,
        sort_by,
    )
}

pub(crate) fn get_metrics_output_function(
    metrics: &Vec<RootMetrics>,
    files_ignored: &[String],
    complex_files: &Vec<FunctionMetrics>,
) {
    Text::print_result(metrics, files_ignored.len(), complex_files.len());
}

// Prints the the given  metrics per function,files ignored and complex functions  in a json format
pub(crate) fn print_metrics_to_json_function<A: AsRef<Path>, B: AsRef<Path>>(
    metrics: &Vec<RootMetrics>,
    files_ignored: &[String],
    json_output: A,
    project_folder: B,
    project_coverage: f64,
    sort_by: Sort,
) -> Result<()> {
    debug!("Exporting to json...");
    Text::print_json_to_file(
        metrics,
        files_ignored,
        project_coverage,
        json_output.as_ref(),
        project_folder.as_ref(),
        sort_by,
    )
}

// Prints the the given  metrics per function, files ignored and complex functions  in a json format
pub(crate) fn print_metrics_to_html_function<A: AsRef<Path>, B: AsRef<Path>>(
    metrics: &Vec<RootMetrics>,
    files_ignored: &[String],
    html: A,
    project_folder: B,
    project_coverage: f64,
    sort_by: Sort,
) -> Result<()> {
    debug!("Exporting to HTML...");
    Text::print_html_to_file(
        metrics,
        files_ignored,
        html.as_ref(),
        project_folder.as_ref(),
        project_coverage,
        sort_by,
    )
}
