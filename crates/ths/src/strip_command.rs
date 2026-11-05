//! `ths strip`: writes `name.hs` for each `name.ths` trade script, beside
//! it or below the project's output directory, with the modules it imports
//! bundled in (see `bundle_plan` and `ths_strip::bundle`). A module, a file
//! with an export, is never uploaded, so nothing is written for it. A
//! script importing no values is stripped alone, its lines matching the
//! source. Like `tsc`, it writes even when the files have type errors,
//! unless the project sets `refuse_output_on_errors`.

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use ths_project::file_extensions;
use ths_syntax::line_column;

use ths_strip::{Binding, BundleModule, BundlePart, bundle};

use crate::bundle_plan::{BundlePlan, PlannedFile, plan};
use crate::relative_paths::relative_path;
use crate::shown_paths::{shown, slash_separated};

/// Why a file cannot be stripped.
enum Refusal {
    DeclarationFile,
    NotTypedSource,
}

/// A `.ths` file that is not a declaration file.
pub fn is_strippable(file: &Path) -> bool {
    refusal(file).is_none()
}

/// Why `file` cannot be stripped, if it cannot.
fn refusal(file: &Path) -> Option<Refusal> {
    let name = file.to_string_lossy();
    if name.ends_with(file_extensions::DECLARATION) {
        Some(Refusal::DeclarationFile)
    } else if !name.ends_with(file_extensions::TYPED_SOURCE) {
        Some(Refusal::NotTypedSource)
    } else {
        None
    }
}

/// Strips each of `files` to the path at the same position in
/// `destinations`.
pub fn run(files: &[PathBuf], destinations: &[PathBuf]) -> ExitCode {
    let mut all_succeeded = true;
    for (file, destination) in files.iter().zip(destinations) {
        if let Err(message) = strip_file(file, destination) {
            eprintln!("{message}");
            all_succeeded = false;
        }
    }
    if all_succeeded { ExitCode::SUCCESS } else { ExitCode::FAILURE }
}

/// Strips `file` to `destination`; a module is skipped.
fn strip_file(file: &Path, destination: &Path) -> Result<(), String> {
    if let Some(refusal) = refusal(file) {
        return Err(refusal_message(file, refusal));
    }
    let Some(plan) = plan(file)? else {
        return Ok(());
    };
    write_output(destination, &stripped_script(&plan)?)
}

/// What is printed when `file` is refused.
fn refusal_message(file: &Path, refusal: Refusal) -> String {
    let shown_file = shown(file);
    let display = shown_file.display();
    match refusal {
        Refusal::DeclarationFile => format!("{display}: declaration files are not stripped"),
        Refusal::NotTypedSource => format!("{display}: not a {} file", file_extensions::TYPED_SOURCE),
    }
}

/// The planned script stripped alone, or bundled with the modules it
/// imports.
fn stripped_script(plan: &BundlePlan) -> Result<String, String> {
    if plan.modules.is_empty() {
        return stripped(&plan.script, ths_strip::strip);
    }
    let base = plan.script.path.parent().unwrap_or(Path::new(""));
    let modules = plan.modules.iter().map(|module| bundle_module(module, base)).collect::<Result<Vec<_>, _>>()?;
    Ok(bundle(&modules, &bundle_part(&plan.script, base)?))
}

/// Writes `output` to `destination`, creating its directory.
fn write_output(destination: &Path, output: &str) -> Result<(), String> {
    let shown_destination = shown(destination);
    if let Some(directory) = destination.parent() {
        std::fs::create_dir_all(directory)
            .map_err(|error| format!("{}: cannot create: {error}", directory.display()))?;
    }
    std::fs::write(destination, output)
        .map_err(|error| format!("{}: cannot write: {error}", shown_destination.display()))
}

type Stripper = fn(&str) -> Result<String, Vec<ths_syntax::ParseError>>;

/// The file stripped, or its syntax errors.
fn stripped(file: &PlannedFile, strip: Stripper) -> Result<String, String> {
    let shown_file = shown(&file.path);
    let display = shown_file.display();
    strip(&file.source).map_err(|errors| {
        errors
            .iter()
            .map(|error| {
                let position = line_column(&file.source, error.range.start());
                format!("{display}:{}:{}: {}", position.line, position.column, error.message)
            })
            .collect::<Vec<_>>()
            .join("\n")
    })
}

/// `file` for a bundle, labelled with its path from `base`, the script's
/// directory, so the bundle names no directory outside the project.
fn bundle_part(file: &PlannedFile, base: &Path) -> Result<BundlePart, String> {
    let value_imports = file.outline.imports.iter().filter(|import| !import.values.is_empty());
    let bindings = value_imports
        .zip(&file.imported_modules)
        .flat_map(|(import, module)| {
            import.values.iter().map(|value| Binding {
                local: value.local.clone(),
                module: *module,
                exported: value.exported.clone(),
            })
        })
        .collect();
    let label = slash_separated(&relative_path(&file.path, base));
    Ok(BundlePart { label, code: stripped(file, ths_strip::strip_compact)?, bindings })
}

fn bundle_module(file: &PlannedFile, base: &Path) -> Result<BundleModule, String> {
    let exports =
        file.outline.exported_values.iter().map(|value| (value.exported.clone(), value.local.clone())).collect();
    Ok(BundleModule { part: bundle_part(file, base)?, exports })
}
