use std::fs;
use std::path::Path;

use minijinja::{context, path_loader, Environment};
use serde::Serialize;

use crate::concurrent::{files::FileMetrics, Metrics, ProjectMetrics, WccOutput};
use crate::{error::*, Complexity, Mode, Thresholds};

pub(crate) trait WccPrinter {
    type Output;

    fn print(self) -> Self::Output;
}   

#[derive(Serialize)]
struct JsonOutput<'a> {
    project_path: &'a Path,
    mode: Mode,
    complexity: &'a Complexity,
    ignored_files_number: usize,
    complex_files_number: usize,
    files: &'a [FileMetrics],
    project: &'a ProjectMetrics,
    complex_files: Vec<&'a str>,
    ignored_files: &'a [String],
}

impl<'a> JsonOutput<'a> {
    fn new(
        project_path: &'a Path,
        mode: Mode,
        complexity: &'a Complexity,
        ignored_files_number: usize,
        complex_files_number: usize,
        files: &'a [FileMetrics],
        project: &'a ProjectMetrics,
        complex_files: Vec<&'a str>,
        ignored_files: &'a [String],
    ) -> Self {
        Self {
            project_path,
            ignored_files_number,
            complex_files_number,
            files,
            project,
            complex_files,
            ignored_files,
            mode,
            complexity,
        }
    }
}

pub(crate) struct JsonPrinter<'a, A: AsRef<Path>, B: AsRef<Path>> {
    project_path: A,
    wcc_output: &'a WccOutput,
    output_path: B,
    mode: Mode,
    complexity: &'a Complexity,
}

impl<'a, A: AsRef<Path>, B: AsRef<Path>> JsonPrinter<'a, A, B> {
    pub(crate) fn new(
        project_path: A,
        wcc_output: &'a WccOutput,
        output_path: B,
        mode: Mode,
        complexity: &'a Complexity,
    ) -> Self {
        Self {
            project_path,
            wcc_output,
            output_path,
            mode,
            complexity,
        }
    }

    fn get_complex_files(&self) -> Vec<&str> {
        self.wcc_output
            .files
            .iter()
            .filter_map(|f| {
                if let Some(true) = f.metrics.is_complex {
                    Some(f.name.as_str())
                } else {
                    None
                }
            })
            .collect()
    }

    fn format_output(&self) -> JsonOutput<'_> {
        let complex_files = self.get_complex_files();

        JsonOutput::new(
            self.project_path.as_ref(),
            self.mode,
            self.complexity,
            self.wcc_output.ignored_files.len(),
            complex_files.len(),
            &self.wcc_output.files,
            &self.wcc_output.project,
            complex_files,
            &self.wcc_output.ignored_files,
        )
    }
}

impl<A: AsRef<Path>, B: AsRef<Path>> WccPrinter for JsonPrinter<'_, A, B> {
    type Output = Result<()>;

    fn print(self) -> Self::Output {
        let output = self.format_output();
        let json = serde_json::to_string_pretty(&output)?;
        fs::write(self.output_path, json.as_bytes())?;

        Ok(())
    }
}

pub(crate) struct HtmlPrinter<'a, P: AsRef<Path>> {
    wcc_output: &'a WccOutput,
    output_path: P,
    mode: Mode,
    complexity: &'a (Complexity, Thresholds),
}

impl<'a, P: AsRef<Path>> HtmlPrinter<'a, P> {
    pub(crate) fn new(
        wcc_output: &'a WccOutput,
        output_path: P,
        mode: Mode,
        complexity: &'a (Complexity, Thresholds),
    ) -> Self {
        Self {
            wcc_output,
            output_path,
            mode,
            complexity,
        }
    }

    fn format_project_metrics(&self) -> Vec<(&'static str, &Metrics)> {
        let mut project_metrics = Vec::new();
        project_metrics.push(("Total", &self.wcc_output.project.total));
        project_metrics.push(("Min", &self.wcc_output.project.min));
        project_metrics.push(("Max", &self.wcc_output.project.max));
        project_metrics.push(("Average", &self.wcc_output.project.average));

        project_metrics
    }

    fn format_files(&self) -> Vec<(String, &FileMetrics)> {
        self.wcc_output
            .files
            .iter()
            .enumerate()
            .map(|(file_number, metrics)| {
                (format!("file_{}.html", file_number + 1), metrics)
            })
            .collect()
    }

    fn print_files(&self, env: &Environment) -> Result<String> {
        let template = env.get_template("files.html")?;
        let output = template.render(context! { files => self.wcc_output.files, project_metrics => self.format_project_metrics(), mode => "Files", complexity => self.complexity.0, thresholds => self.complexity.1.0, ignored_files => self.wcc_output.ignored_files })?;

        Ok(output)
    }

    fn print_functions(&self, env: &Environment) -> Result<String> {
        let files = self.format_files();
        let file_template = env.get_template("file.html")?;
        for file in &files {
            let file_output = file_template.render(context! { file => file.1, mode => "Functions", complexity => self.complexity.0, thresholds => self.complexity.1.0 })?;
            std::fs::write(self.output_path.as_ref().join(&file.0), file_output)?;
        }

        let template = env.get_template("functions.html")?;
        let output = template.render(context! { files => files, project_metrics => self.format_project_metrics(), mode => "Functions", complexity => self.complexity.0, thresholds => self.complexity.1.0, ignored_files => self.wcc_output.ignored_files })?;

        Ok(output)
    }
}

impl<'a, P: AsRef<Path>> WccPrinter for HtmlPrinter<'a, P> {
    type Output = Result<()>;

    fn print(self) -> Self::Output {
        let mut env = Environment::new();
        env.set_loader(path_loader("templates"));
        let output = match self.mode {
            Mode::Files => self.print_files(&env)?,
            Mode::Functions => self.print_functions(&env)?,
        };

        std::fs::write(self.output_path.as_ref().join("index.html"), output)?;

        Ok(())
    }
}
