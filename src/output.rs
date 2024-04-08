use std::fs;
use std::path::Path;

use minijinja::{context, Environment};
use serde::Serialize;

use crate::concurrent::{files::FileMetrics, ProjectMetrics, WccOutput};
use crate::metrics::MetricsThresholds;
use crate::{error::*, Complexity, Mode};

static BASE: (&str, &str) = ("base.html", include_str!("../templates/base.html.jinja"));

static FILES: (&str, &str) = ("files.html", include_str!("../templates/files.html.jinja"));

static FILE_DETAILS: (&str, &str) = (
    "file_details.html",
    include_str!("../templates/file_details.html.jinja"),
);

static NAVBAR: (&str, &str) = (
    "navbar.html",
    include_str!("../templates/navbar.html.jinja"),
);

static STYLE: (&str, &str) = ("style.css", include_str!("../templates/css/style.css"));

static TOOLTIPS: (&str, &str) = ("tooltips.js", include_str!("../templates/js/tooltips.js"));

static COMPLEXITY: (&str, &str) = (
    "complexity.js",
    include_str!("../templates/js/complexity.js"),
);

pub(crate) trait WccPrinter {
    type Output;

    fn print(self) -> Self::Output;
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct JsonOutput<'a> {
    project: &'a Path,
    mode: Mode,
    thresholds: MetricsThresholds,
    files: &'a [FileMetrics],
    project_metrics: &'a ProjectMetrics,
    complex_files_cyclomatic: Vec<&'a str>,
    complex_files_cognitive: Vec<&'a str>,
    ignored_files: &'a [String],
}
pub(crate) struct JsonPrinter<'a> {
    pub(crate) project_path: &'a Path,
    pub(crate) wcc_output: &'a WccOutput,
    pub(crate) output_path: &'a Path,
    pub(crate) mode: Mode,
    pub(crate) thresholds: MetricsThresholds,
}

impl JsonPrinter<'_> {
    fn get_complex_files(&self, complexity: Complexity) -> Vec<&str> {
        self.wcc_output
            .files
            .iter()
            .filter(|f| match complexity {
                Complexity::Cyclomatic => f.metrics.cyclomatic.is_complex.unwrap_or(true),
                Complexity::Cognitive => f.metrics.cyclomatic.is_complex.unwrap_or(true),
            })
            .map(|f| f.name.as_str())
            .collect()
    }

    fn format_output(&self) -> JsonOutput<'_> {
        let complex_files_cyclomatic = self.get_complex_files(Complexity::Cyclomatic);
        let complex_files_cognitive = self.get_complex_files(Complexity::Cognitive);

        JsonOutput {
            project: self.project_path,
            mode: self.mode,
            thresholds: self.thresholds,
            files: &self.wcc_output.files,
            project_metrics: &self.wcc_output.project,
            complex_files_cyclomatic,
            complex_files_cognitive,
            ignored_files: &self.wcc_output.ignored_files,
        }
    }
}

impl WccPrinter for JsonPrinter<'_> {
    type Output = Result<()>;

    fn print(self) -> Self::Output {
        let output = self.format_output();
        let json = serde_json::to_string(&output)?;
        fs::write(self.output_path, json.as_bytes())?;

        Ok(())
    }
}

#[derive(Default)]
struct ComplexSpaces {
    complex_cyclomatic: usize,
    not_complex_cyclomatic: usize,
    complex_cognitive: usize,
    not_complex_cognitive: usize,
}

pub(crate) struct HtmlPrinter<'a> {
    pub(crate) wcc_output: &'a WccOutput,
    pub(crate) output_path: &'a Path,
    pub(crate) mode: Mode,
    pub(crate) thresholds: MetricsThresholds,
}

impl HtmlPrinter<'_> {
    fn format_files(&self) -> Vec<(Option<String>, &FileMetrics)> {
        self.wcc_output
            .files
            .iter()
            .enumerate()
            .map(|(file_number, file)| {
                if file.functions.is_some() {
                    (Some(format!("file_{}.html", file_number + 1)), file)
                } else {
                    (None, file)
                }
            })
            .collect()
    }

    fn get_complex_files(&self) -> ComplexSpaces {
        let mut complex_spaces = ComplexSpaces::default();
        self.wcc_output.files.iter().for_each(|f| {
            if let Some(true) = f.metrics.cyclomatic.is_complex {
                complex_spaces.complex_cyclomatic += 1;
            };
            if let Some(true) = f.metrics.cognitive.is_complex {
                complex_spaces.complex_cognitive += 1;
            };
        });
        complex_spaces.not_complex_cyclomatic =
            self.wcc_output.files.len() - complex_spaces.complex_cyclomatic;
        complex_spaces.not_complex_cognitive =
            self.wcc_output.files.len() - complex_spaces.complex_cognitive;

        complex_spaces
    }

    fn get_complex_functions(&self, file: &FileMetrics) -> ComplexSpaces {
        let mut complex_spaces = ComplexSpaces::default();
        if let Some(functions) = &file.functions {
            functions.iter().for_each(|f| {
                if let Some(true) = f.metrics.cyclomatic.is_complex {
                    complex_spaces.complex_cyclomatic += 1;
                };
                if let Some(true) = f.metrics.cognitive.is_complex {
                    complex_spaces.complex_cognitive += 1;
                };
            });
            complex_spaces.not_complex_cyclomatic =
                functions.len() - complex_spaces.complex_cyclomatic;
            complex_spaces.not_complex_cognitive =
                functions.len() - complex_spaces.complex_cognitive;
        }

        complex_spaces
    }

    fn print_file_details(
        &self,
        env: &mut Environment,
        files: &[(Option<String>, &FileMetrics)],
    ) -> Result<()> {
        env.add_template(FILE_DETAILS.0, FILE_DETAILS.1)?;
        let file_template = env.get_template(FILE_DETAILS.0)?;
        for f in files {
            if let Some(html) = &f.0 {
                let complex_functions = self.get_complex_functions(f.1);
                let file_output = file_template.render(context! {
                    file => f.1,
                    not_complex_cyclomatic => complex_functions.not_complex_cyclomatic,
                    complex_cyclomatic => complex_functions.complex_cyclomatic,
                    not_complex_cognitive => complex_functions.not_complex_cognitive,
                    complex_cognitive => complex_functions.complex_cognitive,
                    mode => "Functions",
                    thresholds => self.thresholds,
                    navbar_brand_href => "index.html",
                })?;
                std::fs::write(self.output_path.join(html), file_output)?;
            }
        }

        Ok(())
    }
}

impl WccPrinter for HtmlPrinter<'_> {
    type Output = Result<()>;

    fn print(self) -> Self::Output {
        let mut env = Environment::new();
        env.add_template(BASE.0, BASE.1)?;
        env.add_template(NAVBAR.0, NAVBAR.1)?;
        env.add_template(STYLE.0, STYLE.1)?;
        env.add_template(TOOLTIPS.0, TOOLTIPS.1)?;
        env.add_template(COMPLEXITY.0, COMPLEXITY.1)?;
        let files = self.format_files();
        if let Mode::Functions = self.mode {
            self.print_file_details(&mut env, &files)?;
        }

        env.add_template(FILES.0, FILES.1)?;
        let template = env.get_template(FILES.0)?;
        let complex_files = self.get_complex_files();
        let output = template.render(context! {
            files => files,
            ignored_files => self.wcc_output.ignored_files,
            ignored_files_num => self.wcc_output.ignored_files.len(),
            not_complex_cyclomatic => complex_files.not_complex_cyclomatic,
            complex_cyclomatic => complex_files.complex_cyclomatic,
            not_complex_cognitive => complex_files.not_complex_cognitive,
            complex_cognitive => complex_files.complex_cognitive,
            project => self.wcc_output.project,
            mode => self.mode,
            thresholds => self.thresholds,
        })?;

        std::fs::write(self.output_path.join("index.html"), output)?;

        Ok(())
    }
}
