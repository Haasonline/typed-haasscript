mod bundle_plan;
mod check_command;
mod check_options;
mod file_check;
mod file_selection;
mod init_command;
mod language_server;
mod module_graph;
mod module_interfaces;
mod project_declarations;
mod relative_paths;
mod shown_paths;
mod source_dialect;
mod source_files;
mod strip_command;

use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Args, Parser, Subcommand};
use ths_check::CheckOptions;
use ths_project::{Project, is_source_file, stripped_beside};

use crate::check_options::options_from_settings;
use crate::file_check::Context;
use crate::file_selection::{Selection, select};
use crate::module_graph::ModuleGraph;
use crate::module_interfaces::{InterfaceContext, ModuleInterfaces};
use crate::project_declarations::ProjectDeclarations;
use crate::source_files::Disk;

#[derive(Parser)]
#[command(name = "ths", version, about = "Typed HaasScript: type checker, stripper and language server")]
struct CommandLine {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Check .ths files for syntax and type errors; .lua and .hs files report only what the server rejects.
    Check {
        /// Report implicit `any` in .ths files: unannotated parameters and variables whose type cannot be inferred
        #[arg(long)]
        strict: bool,
        /// Warn where a name falls back to an outer or global definition because a `local` of the same name ended with its block
        #[arg(long)]
        shadowing: bool,
        #[command(flatten)]
        files: FileArguments,
    },
    /// Remove type annotations from .ths files, writing a .hs file for each.
    Strip {
        #[command(flatten)]
        files: FileArguments,
    },
    /// Run the language server, talking the Language Server Protocol over standard input and output.
    Server,
    /// Make a directory a project by writing a ths.toml there; editors check only files in a project.
    Init {
        /// The directory; by default the current one
        #[arg(default_value = ".")]
        directory: PathBuf,
    },
}

#[derive(Args)]
struct FileArguments {
    /// The ths.toml to use, or a directory holding one; by default the nearest one above the current directory
    #[arg(long)]
    project: Option<PathBuf>,
    /// Files and directories to work on; by default the files the project includes
    paths: Vec<PathBuf>,
}

fn main() -> ExitCode {
    let result = match CommandLine::parse().command {
        Command::Check { strict, shadowing, files } => check(strict, shadowing, &files),
        Command::Strip { files } => strip(&files),
        Command::Server => language_server::run(),
        Command::Init { directory } => init(&directory),
    };
    result.unwrap_or_else(|message| {
        eprintln!("{message}");
        ExitCode::FAILURE
    })
}

fn check(strict: bool, shadowing: bool, files: &FileArguments) -> Result<ExitCode, String> {
    let Selection { project, files } = select(files.project.as_deref(), &files.paths, is_source_file)?;
    let declarations = loaded_declarations(project.as_ref(), &files)?;
    let settings = project.map(|project| project.settings.check).unwrap_or_default();
    let configured = options_from_settings(&settings);
    let options = CheckOptions { strict: strict || configured.strict, shadowing: shadowing || configured.shadowing };
    let has_errors = check_selected(&files, &declarations, options);
    Ok(if has_errors { ExitCode::FAILURE } else { ExitCode::SUCCESS })
}

fn init(directory: &std::path::Path) -> Result<ExitCode, String> {
    let written = init_command::create_settings_file(directory)?;
    println!("Created {}", shown_paths::shown(&written).display());
    Ok(ExitCode::SUCCESS)
}

fn strip(arguments: &FileArguments) -> Result<ExitCode, String> {
    let Selection { project, files } =
        select(arguments.project.as_deref(), &arguments.paths, strip_command::is_strippable)?;
    if let Some(project) = &project
        && project.settings.check.refuse_output_on_errors
        && has_errors(project, arguments)?
    {
        return Err("ths: nothing written: there are errors and refuse_output_on_errors is set".to_string());
    }
    let destinations = match &project {
        Some(project) => project.stripped_paths(&files),
        None => files.iter().map(|file| stripped_beside(file)).collect(),
    };
    Ok(strip_command::run(&files, &destinations))
}

/// Checks every source file the arguments select, printing the
/// diagnostics, as `tsc` does before refusing to emit.
fn has_errors(project: &Project, arguments: &FileArguments) -> Result<bool, String> {
    let checked = select(arguments.project.as_deref(), &arguments.paths, is_source_file)?;
    let declarations = loaded_declarations(Some(project), &checked.files)?;
    let options = options_from_settings(&project.settings.check);
    Ok(check_selected(&checked.files, &declarations, options))
}

/// Checks `files`, each seeing the modules it imports; returns whether
/// any has an error.
fn check_selected(files: &[PathBuf], declarations: &ProjectDeclarations, options: CheckOptions) -> bool {
    let modules = ModuleGraph::build(files, &Disk);
    let interfaces = ModuleInterfaces::build(&modules, &InterfaceContext { declarations, options, sources: &Disk });
    check_command::check_files(files, &Context { declarations, modules: &modules, interfaces: &interfaces, options })
}

/// The project's declarations, telling about commands defined twice.
fn loaded_declarations(project: Option<&Project>, files: &[PathBuf]) -> Result<ProjectDeclarations, String> {
    let loaded = project_declarations::load(project, files, &Disk)?;
    for duplicate in &loaded.duplicates {
        eprintln!("{duplicate}");
    }
    Ok(loaded)
}
