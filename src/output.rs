use std::fs::File;
use std::path::*;

use csv;
use serde::{Deserialize, Serialize};
use tracing::debug;

use crate::error::*;
use crate::files::FileMetrics;
use crate::functions::{FunctionMetrics, RootMetrics};
use crate::utility::Sort;

// Struct for JSON for files
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct JSONOutput {
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
pub struct JSONOutputFunc {
    project_folder: String,
    number_of_files_ignored: usize,
    number_of_complex_functions: usize,
    files: Vec<RootMetrics>,
    files_ignored: Vec<String>,
    complex_functions: Vec<FunctionMetrics>,
    project_coverage: f64,
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
    fn print_csv_to_file(
        result: &T,
        files_ignored: &[String],
        project_coverage: f64,
        csv_path: &Path,
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
        println!("FILES IGNORED: {}", files_ignored);
        println!("COMPLEX FILES: {}", complex_files);
    }
    fn print_csv_to_file(
        result: &Vec<FileMetrics>,
        files_ignored: &[String],
        project_coverage: f64,
        csv_path: &Path,
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
        let mut writer = csv::Writer::from_path(csv_path)?;
        writer.write_record(&[
            "FILE",
            "WCC PLAIN",
            "WCC QUANTIZED",
            "CRAP",
            "SKUNK",
            "IGNORED",
            "IS COMPLEX",
            "FILE PATH",
        ])?;
        result.iter().try_for_each(|m| -> Result<()> {
            writer.write_record(&[
                &m.file,
                &format!("{:.3}", m.metrics.wcc_plain),
                &format!("{:.3}", m.metrics.wcc_quantized),
                &format!("{:.3}", m.metrics.crap),
                &format!("{:.3}", m.metrics.skunk),
                &format!("{}", false),
                &format!("{}", m.metrics.is_complex),
                &m.file_path,
            ])?;
            Ok(())
        })?;
        writer.write_record(&[
            "PROJECT_COVERAGE",
            format!("{:.3}", project_coverage).as_str(),
            "-",
            "-",
            "-",
            "-",
            "-",
            "-",
        ])?;
        writer.write_record(&[
            "LIST OF COMPLEX FILES",
            "----------",
            "----------",
            "----------",
            "----------",
            "----------",
            "----------",
            "----------",
        ])?;
        complex_files.iter().try_for_each(|m| -> Result<()> {
            writer.write_record(&[
                &m.file,
                &format!("{:.3}", m.metrics.wcc_plain),
                &format!("{:.3}", m.metrics.wcc_quantized),
                &format!("{:.3}", m.metrics.crap),
                &format!("{:.3}", m.metrics.skunk),
                &format!("{}", false),
                &format!("{}", m.metrics.is_complex),
                &m.file_path,
            ])?;
            Ok(())
        })?;
        writer.write_record(&[
            "TOTAL COMPLEX FILES",
            format!("{:?}", complex_files.len()).as_str(),
            "",
            "",
            "",
            "",
            "",
            "",
        ])?;
        writer.write_record(&[
            "LIST OF IGNORED FILES",
            "----------",
            "----------",
            "----------",
            "----------",
            "----------",
            "----------",
            "----------",
        ])?;
        files_ignored.iter().try_for_each(|file| -> Result<()> {
            writer.write_record(&[
                file.as_str(),
                format!("{:.3}", 0.).as_str(),
                format!("{:.3}", 0.).as_str(),
                format!("{:.3}", 0.).as_str(),
                format!("{:.3}", 0.).as_str(),
                format!("{}", true).as_str(),
                "-",
                "-",
            ])?;
            Ok(())
        })?;
        writer.write_record(&[
            "TOTAL FILES IGNORED",
            format!("{:?}", files_ignored.len()).as_str(),
            "",
            "",
            "",
            "",
            "",
            "",
        ])?;
        writer.flush()?;
        Ok(())
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
        println!("FILES IGNORED: {}", files_ignored);
        println!("COMPLEX FUNCTIONS: {}", complex_files);
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
    fn print_csv_to_file(
        result: &Vec<RootMetrics>,
        files_ignored: &[String],
        project_coverage: f64,
        csv_path: &Path,
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
        let mut writer = csv::Writer::from_path(csv_path)?;
        writer.write_record(&[
            "FUNCTION",
            "WCC PLAIN",
            "WCC QUANTIZED",
            "CRAP",
            "SKUNK",
            "IGNORED",
            "IS COMPLEX",
            "FILE PATH",
        ])?;
        result.iter().try_for_each(|m| -> Result<()> {
            writer.write_record(&[
                &m.file_name,
                &format!("{:.3}", m.metrics.wcc_plain),
                &format!("{:.3}", m.metrics.wcc_quantized),
                &format!("{:.3}", m.metrics.crap),
                &format!("{:.3}", m.metrics.skunk),
                &format!("{}", false),
                &format!("{}", m.metrics.is_complex),
                &m.file_path,
            ])?;
            m.functions.iter().try_for_each(|m| -> Result<()> {
                writer.write_record(&[
                    &m.function_name,
                    &format!("{:.3}", m.metrics.wcc_plain),
                    &format!("{:.3}", m.metrics.wcc_quantized),
                    &format!("{:.3}", m.metrics.crap),
                    &format!("{:.3}", m.metrics.skunk),
                    &format!("{}", false),
                    &format!("{}", m.metrics.is_complex),
                    &m.function_path,
                ])?;
                Ok(())
            })?;
            Ok(())
        })?;
        writer.write_record(&[
            "PROJECT_COVERAGE",
            format!("{:.3}", project_coverage).as_str(),
            "-",
            "-",
            "-",
            "-",
            "-",
            "-",
        ])?;
        writer.write_record(&[
            "LIST OF COMPLEX FUNCTIONS",
            "----------",
            "----------",
            "----------",
            "----------",
            "----------",
            "----------",
            "----------",
        ])?;
        complex_functions.iter().try_for_each(|m| -> Result<()> {
            writer.write_record(&[
                &m.function_name,
                &format!("{:.3}", m.metrics.wcc_plain),
                &format!("{:.3}", m.metrics.wcc_quantized),
                &format!("{:.3}", m.metrics.crap),
                &format!("{:.3}", m.metrics.skunk),
                &format!("{}", false),
                &format!("{}", m.metrics.is_complex),
                &m.function_path,
            ])?;
            Ok(())
        })?;
        writer.write_record(&[
            "TOTAL COMPLEX FUNCTIONS",
            format!("{:?}", complex_functions.len()).as_str(),
            "",
            "",
            "",
            "",
            "",
            "",
        ])?;
        writer.write_record(&[
            "LIST OF IGNORED FILES",
            "----------",
            "----------",
            "----------",
            "----------",
            "----------",
            "----------",
            "----------",
        ])?;
        files_ignored.iter().try_for_each(|file| -> Result<()> {
            writer.write_record(&[
                file.as_str(),
                format!("{:.3}", 0.).as_str(),
                format!("{:.3}", 0.).as_str(),
                format!("{:.3}", 0.).as_str(),
                format!("{:.3}", 0.).as_str(),
                format!("{}", true).as_str(),
                "-",
                "-",
            ])?;
            Ok(())
        })?;
        writer.write_record(&[
            "TOTAL FILES IGNORED",
            format!("{:?}", files_ignored.len()).as_str(),
            "",
            "",
            "",
            "",
            "",
            "",
        ])?;
        writer.flush()?;
        Ok(())
    }
}

// Export all metrics to a json file
pub fn export_to_json(
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
pub fn export_to_json_function(
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

/// This Function get the folder of the repo to analyzed and the path to the json obtained using grcov
/// It prints all the WCC, CRAP and SkunkScore values for all the files in the folders
/// the output will be print as follows:
/// FILE       | WCC PLAIN | WCC QUANTIZED | CRAP       | SKUNKSCORE | "IS_COMPLEX" | "PATH"
/// if the a file is not found in the json that files will be skipped

pub fn get_metrics_output(
    metrics: &Vec<FileMetrics>,
    files_ignored: &Vec<String>,
    complex_files: &Vec<FileMetrics>,
) {
    Text::print_result(metrics, files_ignored.len(), complex_files.len());
}

/// Prints the the given  metrics ,files ignored and complex files  in a csv format
/// The structure is the following :
/// "FILE","WCC PLAIN","WCC QUANTIZED","CRAP","SKUNK","IGNORED","IS COMPLEX","FILE PATH",
pub fn print_metrics_to_csv<A: AsRef<Path> + Copy>(
    metrics: &Vec<FileMetrics>,
    files_ignored: &[String],
    csv_path: A,
    project_coverage: f64,
    sort_by: Sort,
) -> Result<()> {
    debug!("Exporting to csv...");
    Text::print_csv_to_file(
        metrics,
        files_ignored,
        project_coverage,
        csv_path.as_ref(),
        sort_by,
    )
}

/// Prints the the given  metrics ,files ignored and complex files  in a json format
pub fn print_metrics_to_json<A: AsRef<Path> + Copy>(
    metrics: &Vec<FileMetrics>,
    files_ignored: &[String],
    json_output: A,
    project_folder: A,
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

pub fn get_metrics_output_function(
    metrics: &Vec<RootMetrics>,
    files_ignored: &[String],
    complex_files: &Vec<FunctionMetrics>,
) {
    Text::print_result(metrics, files_ignored.len(), complex_files.len());
}

/// Prints the the given  metrics per function ,files ignored and complex function  in a csv format
/// The structure is the following :
/// "FUNCTION","WCC PLAIN","WCC QUANTIZED","CRAP","SKUNK","IGNORED","IS COMPLEX","FILE PATH",
pub fn print_metrics_to_csv_function<A: AsRef<Path> + Copy>(
    metrics: &Vec<RootMetrics>,
    files_ignored: &[String],
    csv_path: A,
    project_coverage: f64,
    sort_by: Sort,
) -> Result<()> {
    debug!("Exporting to csv...");
    Text::print_csv_to_file(
        metrics,
        files_ignored,
        project_coverage,
        csv_path.as_ref(),
        sort_by,
    )
}

/// Prints the the given  metrics per function,files ignored and complex functions  in a json format
pub fn print_metrics_to_json_function<A: AsRef<Path> + Copy>(
    metrics: &Vec<RootMetrics>,
    files_ignored: &[String],
    json_output: A,
    project_folder: A,
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::files::*;
    use crate::functions::*;
    use crate::utility::*;
    use std::fs;
    use std::path::Path;

    const JSON: &str = "./data/seahorse/seahorse.json";
    const FOLDER: &str = "./data/test_project/";

    #[test]
    fn test_file_csv() {
        let json = Path::new(JSON);
        let (metrics, files_ignored, _complex_files, project_coverage) = get_metrics_concurrent(
            "./data/test_project/",
            json,
            Complexity::Cyclomatic,
            8,
            &[30., 1.5, 35., 30.],
            Sort::WccPlain,
        )
        .unwrap();
        Text::print_csv_to_file(
            &metrics,
            &files_ignored,
            project_coverage,
            Path::new("./data/test_project/to_compare.csv"),
            Sort::WccPlain,
        )
        .unwrap();
        let to_compare = fs::read_to_string("./data/test_project/to_compare.csv").unwrap();
        let expected = fs::read_to_string("./data/test_project/test.csv")
            .unwrap()
            .replace('\r', "");
        assert!(to_compare == expected);
        fs::remove_file("./data/test_project/to_compare.csv").unwrap();
    }

    #[test]
    fn test_file_json() {
        let json = Path::new(JSON);
        let path = Path::new(FOLDER);
        let (metrics, files_ignored, complex_files, project_coverage) = get_metrics_concurrent(
            "./data/test_project/",
            json,
            Complexity::Cyclomatic,
            8,
            &[30., 1.5, 35., 30.],
            Sort::WccPlain,
        )
        .unwrap();
        let to_compare = export_to_json(
            path,
            &metrics,
            &files_ignored,
            &complex_files,
            project_coverage,
        );
        let expected = JSONOutput {
            project_folder: "./data/test_project/".into(),
            number_of_files_ignored: 0,
            number_of_complex_files: 1,
            metrics: vec![
                FileMetrics {
                    metrics: Metrics {
                        wcc_plain: 34.696335078534034,
                        wcc_quantized: 0.7382198952879581,
                        crap: 48.32881221072737,
                        skunk: 15.87012987012987,
                        is_complex: true,
                        coverage: 91.56,
                    },
                    file: "flag.rs".into(),
                    file_path: "src/flag.rs".into(),
                },
                FileMetrics {
                    metrics: Metrics {
                        wcc_plain: 34.696335078534034,
                        wcc_quantized: 0.7382198952879581,
                        crap: 48.32881221072737,
                        skunk: 15.87012987012987,
                        is_complex: false,
                        coverage: 91.55844155844156,
                    },
                    file: "PROJECT".into(),
                    file_path: "-".into(),
                },
                FileMetrics {
                    metrics: Metrics {
                        wcc_plain: 34.696335078534034,
                        wcc_quantized: 0.7382198952879581,
                        crap: 48.32881221072737,
                        skunk: 15.87012987012987,
                        is_complex: false,
                        coverage: 91.56,
                    },
                    file: "AVG".into(),
                    file_path: "-".into(),
                },
                FileMetrics {
                    metrics: Metrics {
                        wcc_plain: 34.696335078534034,
                        wcc_quantized: 0.7382198952879581,
                        crap: 48.32881221072737,
                        skunk: 15.87012987012987,
                        is_complex: false,
                        coverage: 0.0,
                    },
                    file: "MAX".into(),
                    file_path: "-".into(),
                },
                FileMetrics {
                    metrics: Metrics {
                        wcc_plain: 34.696335078534034,
                        wcc_quantized: 0.7382198952879581,
                        crap: 48.32881221072737,
                        skunk: 15.87012987012987,
                        is_complex: false,
                        coverage: 100.0,
                    },
                    file: "MIN".into(),
                    file_path: "-".into(),
                },
            ],
            files_ignored: Vec::<String>::new(),
            complex_files: vec![FileMetrics {
                metrics: Metrics {
                    wcc_plain: 34.696335078534034,
                    wcc_quantized: 0.7382198952879581,
                    crap: 48.32881221072737,
                    skunk: 15.87012987012987,
                    is_complex: true,
                    coverage: 91.56,
                },
                file: "flag.rs".into(),
                file_path: "src/flag.rs".into(),
            }],
            project_coverage: 91.56,
        };
        assert!(to_compare == expected);
    }

    #[test]
    fn test_functions_csv() {
        let json = Path::new(JSON);
        let (metrics, files_ignored, _complex_files, project_coverage) =
            get_functions_metrics_concurrent(
                "./data/test_project/",
                json,
                Complexity::Cyclomatic,
                8,
                &[30., 1.5, 35., 30.],
                Sort::WccPlain,
            )
            .unwrap();
        Text::print_csv_to_file(
            &metrics,
            &files_ignored,
            project_coverage,
            Path::new("./data/test_project/to_compare_fun.csv"),
            Sort::WccPlain,
        )
        .unwrap();
        let to_compare = fs::read_to_string("./data/test_project/to_compare_fun.csv").unwrap();
        let expected = fs::read_to_string("./data/test_project/test_fun.csv")
            .unwrap()
            .replace('\r', "");
        assert!(to_compare == expected);
        fs::remove_file("./data/test_project/to_compare_fun.csv").unwrap();
    }

    #[test]
    fn test_functions_json() {
        let json = Path::new(JSON);
        let (metrics, files_ignored, complex_files, project_coverage) =
            get_functions_metrics_concurrent(
                "./data/test_project/",
                json,
                Complexity::Cyclomatic,
                8,
                &[30., 1.5, 35., 30.],
                Sort::WccPlain,
            )
            .unwrap();
        let path = Path::new(FOLDER);
        let to_compare = export_to_json_function(
            path,
            &metrics,
            &files_ignored,
            &complex_files,
            project_coverage,
        );
        let expected= JSONOutputFunc {
                project_folder: "./data/test_project/".into(),
                number_of_files_ignored: 0,
                number_of_complex_functions: 0,
                files: vec![
                    RootMetrics {
                        metrics: Metrics {
                            wcc_plain: 34.696335078534034,
                            wcc_quantized: 0.7382198952879581,
                            crap: 48.32881221072737,
                            skunk: 15.87012987012987,
                            is_complex: true,
                            coverage: 91.56
                        },
                        file_name: "flag.rs".into(),
                        file_path: "src/flag.rs".into(),
                        start_line: 1,
                        end_line: 261,
                        functions: vec![
                            FunctionMetrics {
                                metrics: Metrics {
                                    wcc_plain: 0.7619047619047619,
                                    wcc_quantized: 0.7619047619047619,
                                    crap: 1.0,
                                    skunk: 0.0,
                                    is_complex: false,
                                    coverage: 100.0
                                },
                                function_name: "opiton_index (155, 175)".into(),
                                function_path: "/opiton_index (155,175)".into(),
                                start_line: 155,
                                end_line: 175
                            },
                            FunctionMetrics {
                                metrics: Metrics{
                                    wcc_plain: 1.0,
                                    wcc_quantized: 1.0,
                                    crap: 1.0,
                                    skunk: 0.0,
                                    is_complex: false,
                                    coverage: 100.0
                                },
                                function_name: "construct_fail_1 (179, 181)".into(),
                                function_path: "/construct_fail_1 (179,181)".into(),
                                start_line: 179,
                                end_line: 181
                            },
                            FunctionMetrics {
                                metrics: Metrics{
                                    wcc_plain: 1.0,
                                    wcc_quantized: 1.0,
                                    crap: 1.0,
                                    skunk: 0.0,
                                    is_complex: false,
                                    coverage: 100.0
                                },
                                function_name: "construct_fail_2 (185, 187)".into(),
                                function_path: "/construct_fail_2 (185,187)".into(),
                                start_line: 185,
                                end_line: 187
                            },
                            FunctionMetrics {
                                metrics: Metrics {
                                    wcc_plain: 1.0,
                                    wcc_quantized: 1.0,
                                    crap: 1.0,
                                    skunk: 0.0,
                                    is_complex: false,
                                    coverage: 100.0
                                },
                                function_name: "construct_fail_3 (191, 193)".into(),
                                function_path: "/construct_fail_3 (191,193)".into(),
                                start_line: 191,
                                end_line: 193
                            },
                            FunctionMetrics {
                                metrics: Metrics {
                                    wcc_plain: 2.769230769230769,
                                    wcc_quantized: 0.9230769230769231,
                                    crap: 3.0040964952207556,
                                    skunk: 0.9230769230769231,
                                    is_complex: false,
                                    coverage: 92.31
                                },
                                function_name: "bool_flag_test (196, 209)".into(),
                                function_path: "/bool_flag_test (196,209)".into(),
                                start_line: 196,
                                end_line: 209
                            },
                            FunctionMetrics {
                                metrics: Metrics{
                                    wcc_plain: 2.7857142857142856,
                                    wcc_quantized: 0.9285714285714286,
                                    crap: 3.003279883381924,
                                    skunk: 0.8571428571428567,
                                    is_complex: false,
                                    coverage: 92.86
                                },
                                function_name: "string_flag_test (212, 226)".into(),
                                function_path: "/string_flag_test (212,226)".into(),
                                start_line: 212,
                                end_line: 226
                            },
                            FunctionMetrics {
                                metrics: Metrics {
                                    wcc_plain: 2.7857142857142856,
                                    wcc_quantized: 0.9285714285714286,
                                    crap: 3.003279883381924,
                                    skunk: 0.8571428571428567,
                                    is_complex: false,
                                    coverage: 92.86
                                },
                                function_name: "int_flag_test (229, 243)".into(),
                                function_path: "/int_flag_test (229,243)".into(),
                                start_line: 229,
                                end_line: 243
                            },
                            FunctionMetrics {
                                metrics: Metrics {
                                    wcc_plain: 2.7857142857142856,
                                    wcc_quantized: 0.9285714285714286,
                                    crap: 3.003279883381924,
                                    skunk: 0.8571428571428567,
                                    is_complex: false,
                                    coverage: 92.86
                                },
                                function_name: "float_flag_test (246, 260)".into(),
                                function_path: "/float_flag_test (246,260)".into(),
                                start_line: 246,
                                end_line: 260
                            },
                            FunctionMetrics {
                                metrics: Metrics {
                                    wcc_plain: 4.666666666666667,
                                    wcc_quantized: 1.1666666666666667,
                                    crap: 4.0,
                                    skunk: 0.0,
                                    is_complex: false,
                                    coverage: 100.0
                                },
                                function_name: "new (47, 74)".into(),
                                function_path: "/Flag (36,148)/new (47,74)".into(),
                                start_line: 47,
                                end_line: 74
                            },
                            FunctionMetrics {
                                metrics: Metrics {
                                    wcc_plain: 0.0,
                                    wcc_quantized: 0.0,
                                    crap: 2.0,
                                    skunk: 4.0,
                                    is_complex: false,
                                    coverage: 0.0
                                },
                                function_name: "description (86, 89)".into(),
                                function_path: "/Flag (36,148)/description (86,89)".into(),
                                start_line: 86,
                                end_line: 89
                            },
                            FunctionMetrics {
                                metrics: Metrics {
                                    wcc_plain: 1.5,
                                    wcc_quantized: 0.75,
                                    crap: 2.011661807580175,
                                    skunk: 1.1428571428571435,
                                    is_complex: false,
                                    coverage: 85.71
                                },
                                function_name: "alias (105, 112)".into(),
                                function_path: "/Flag (36,148)/alias (105,112)".into(),
                                start_line: 105,
                                end_line: 112
                            },
                            FunctionMetrics {
                                metrics: Metrics {
                                    wcc_plain: 6.125,
                                    wcc_quantized: 0.875,
                                    crap: 7.0,
                                    skunk: 0.0,
                                    is_complex: false,
                                    coverage: 100.0
                                },
                                function_name: "option_index (115, 122)".into(),
                                function_path: "/Flag (36,148)/option_index (115,122)".into(),
                                start_line: 115,
                                end_line: 122
                            },
                            FunctionMetrics {
                                metrics: Metrics{
                                    wcc_plain: 8.478260869565217,
                                    wcc_quantized: 0.5652173913043478,
                                    crap: 17.93099938937513,
                                    skunk: 14.11764705882353,
                                    is_complex: false,
                                    coverage: 76.47
                                },
                                function_name: "value (125, 147)".into(),
                                function_path: "/Flag (36,148)/value (125,147)".into(),
                                start_line: 125,
                                end_line: 147
                            },
                            FunctionMetrics {
                                metrics: Metrics{
                                    wcc_plain: 3.0,
                                    wcc_quantized: 1.0,
                                    crap: 3.0,
                                    skunk: 0.0,
                                    is_complex: false,
                                    coverage: 100.0
                                },
                                function_name: "<anonymous> (117, 119)".into(),
                                function_path: "/Flag (36,148)/option_index (115,122)/<anonymous> (117,119)".into(),
                                start_line: 117,
                                end_line: 119
                            },
                            FunctionMetrics {
                                metrics: Metrics {
                                    wcc_plain: 1.0,
                                    wcc_quantized: 1.0,
                                    crap: 1.0,
                                    skunk: 0.0,
                                    is_complex: false,
                                    coverage: 100.0
                                },
                                function_name: "<anonymous> (120, 120)".into(),
                                function_path: "/Flag (36,148)/option_index (115,122)/<anonymous> (120,120)".into(),
                                start_line: 120,
                                end_line: 120
                            },
                            FunctionMetrics {
                                metrics: Metrics {
                                    wcc_plain: 1.0,
                                    wcc_quantized: 1.0,
                                    crap: 1.0,
                                    skunk: 0.0,
                                    is_complex: false,
                                    coverage: 100.0
                                },
                                function_name: "<anonymous> (118, 118)".into(),
                                function_path: "/Flag (36,148)/option_index (115,122)/<anonymous> (117,119)/<anonymous> (118,118)".into(),
                                start_line: 118,
                                end_line: 118
                            }
                        ]
                    },
                    RootMetrics {
                        metrics: Metrics {
                            wcc_plain: 34.696335078534034,
                            wcc_quantized: 0.7382198952879581,
                            crap: 48.32881221072737,
                            skunk: 15.87012987012987,
                            is_complex: false,
                            coverage: 91.55844155844156
                        },
                        file_name: "PROJECT".into(),
                        file_path: "-".into(),
                        start_line: 0,
                        end_line: 0,
                        functions: Vec::<FunctionMetrics>::new()
                    },
                    RootMetrics {
                        metrics: Metrics {
                            wcc_plain: 34.696335078534034,
                            wcc_quantized: 0.7382198952879581,
                            crap: 48.32881221072737,
                            skunk: 15.87012987012987,
                            is_complex: false,
                            coverage: 91.56
                        },
                        file_name: "AVG".into(),
                        file_path: "-".into(),
                        start_line: 0,
                        end_line: 0,
                        functions: Vec::<FunctionMetrics>::new()
                    },
                    RootMetrics {
                        metrics: Metrics {
                            wcc_plain: 34.696335078534034,
                            wcc_quantized: 0.7382198952879581,
                            crap: 48.32881221072737,
                            skunk: 15.87012987012987,
                            is_complex: false,
                            coverage: 0.0
                        },
                        file_name: "MAX".into(),
                        file_path: "-".into(),
                        start_line: 0,
                        end_line: 0,
                        functions: Vec::<FunctionMetrics>::new()
                    },
                    RootMetrics {
                        metrics: Metrics {
                            wcc_plain: 34.696335078534034,
                            wcc_quantized: 0.7382198952879581,
                            crap: 48.32881221072737,
                            skunk: 15.87012987012987,
                            is_complex: false,
                            coverage: 100.0
                        },
                        file_name: "MIN".into(),
                        file_path: "-".into(),
                        start_line: 0,
                        end_line: 0,
                        functions: Vec::<FunctionMetrics>::new()
                    }
                ],
                files_ignored: Vec::<String>::new(),
                complex_functions: Vec::<FunctionMetrics>::new(),
                project_coverage: 91.56
        };
        assert!(to_compare == expected);
    }
}
