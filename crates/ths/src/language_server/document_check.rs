//! Checks every open document as `ths check` would check it from the
//! nearest `ths.toml` above it, reading open documents with their unsaved
//! text. Documents of one project are checked together, so they share one
//! loading of the project's declarations and modules.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use lsp_types::{Diagnostic as EditorDiagnostic, DiagnosticSeverity, PublishDiagnosticsParams};
use ths_check::{CheckOptions, Diagnostic, Severity};
use ths_project::{Project, is_stripped_beside_source};
use ths_syntax::Dialect;

use super::document_analyses::DocumentAnalysis;
use super::open_documents::{OpenDocument, OpenDocuments};
use super::positions::{LineIndex, PositionEncoding};
use crate::check_options::options_from_settings;
use crate::file_check::{self, Context};
use crate::module_graph::ModuleGraph;
use crate::module_interfaces::{InterfaceContext, ModuleInterfaces};
use crate::project_declarations::{self, ProjectDeclarations};
use crate::source_dialect::dialect_of;

/// Names the diagnostics in the editor.
const DIAGNOSTIC_SOURCE: &str = "ths";

pub struct CheckedDocuments {
    /// One for every open document; empty for one that is not checked.
    pub published: Vec<PublishDiagnosticsParams>,
    /// What stopped a project from being checked, and commands defined twice.
    pub problems: Vec<String>,
    /// For hover, definitions and completion: one for every checked document.
    pub analyses: Vec<DocumentAnalysis>,
}

/// The open documents of one project.
struct ProjectDocuments<'open> {
    project: Project,
    members: Vec<(&'open PathBuf, &'open OpenDocument)>,
}

pub fn check(documents: &OpenDocuments, encoding: PositionEncoding) -> CheckedDocuments {
    let mut checked = CheckedDocuments { published: Vec::new(), problems: Vec::new(), analyses: Vec::new() };
    let mut projects: BTreeMap<PathBuf, ProjectDocuments> = BTreeMap::new();
    for (path, document) in documents.iter() {
        match Project::find(path.parent().unwrap_or(path)) {
            Err(error) => {
                checked.problems.push(error.to_string());
                checked.published.push(publication(document, Vec::new()));
            }
            // Only a project is checked: the editor plugins offer to create
            // its ths.toml.
            Ok(None) => checked.published.push(publication(document, Vec::new())),
            Ok(Some(project)) if !is_checked(path, &project) => {
                checked.published.push(publication(document, Vec::new()));
            }
            Ok(Some(project)) => {
                let entry = projects
                    .entry(project.directory.clone())
                    .or_insert_with(|| ProjectDocuments { project, members: Vec::new() });
                entry.members.push((path, document));
            }
        }
    }
    for ProjectDocuments { project, members } in projects.into_values() {
        check_project(&project, &members, documents, encoding, &mut checked);
    }
    checked
}

/// Whether `ths check` would check `path`: a source file that is not the
/// stripped output of another.
fn is_checked(path: &Path, project: &Project) -> bool {
    let in_output = project.output_directory().is_some_and(|output| path.starts_with(output));
    dialect_of(path).is_some() && !is_stripped_beside_source(path) && !in_output
}

/// What one project's documents are checked against.
struct ProjectContext {
    declarations: ProjectDeclarations,
    modules: ModuleGraph,
    interfaces: ModuleInterfaces,
    options: CheckOptions,
}

impl ProjectContext {
    /// Loads the project's declarations, modules and interfaces, reading
    /// open documents with their unsaved text.
    fn load(project: &Project, files: &[PathBuf], documents: &OpenDocuments) -> Result<ProjectContext, String> {
        let declarations = project_declarations::load(Some(project), files, documents)?;
        let options = options_from_settings(&project.settings.check);
        let modules = ModuleGraph::build(files, documents);
        let interfaces = ModuleInterfaces::build(
            &modules,
            &InterfaceContext { declarations: &declarations, options, sources: documents },
        );
        Ok(ProjectContext { declarations, modules, interfaces, options })
    }

    fn context(&self) -> Context<'_> {
        Context {
            declarations: &self.declarations,
            modules: &self.modules,
            interfaces: &self.interfaces,
            options: self.options,
        }
    }
}

fn check_project(
    project: &Project,
    members: &[(&PathBuf, &OpenDocument)],
    documents: &OpenDocuments,
    encoding: PositionEncoding,
    checked: &mut CheckedDocuments,
) {
    let files: Vec<PathBuf> = members.iter().map(|(path, _)| (*path).clone()).collect();
    let project_context = match ProjectContext::load(project, &files, documents) {
        Ok(project_context) => project_context,
        Err(problem) => {
            checked.problems.push(problem);
            checked.published.extend(members.iter().map(|(_, document)| publication(document, Vec::new())));
            return;
        }
    };
    checked.problems.extend(project_context.declarations.duplicates.iter().cloned());
    let context = project_context.context();
    for (path, document) in members {
        let Some(dialect) = dialect_of(path) else { continue };
        let (published, analysis) = checked_document(path, document, dialect, &context, encoding);
        checked.published.push(published);
        checked.analyses.push(analysis);
    }
}

/// The diagnostics of one open document, ready to publish, and its
/// analysis.
fn checked_document(
    path: &Path,
    document: &OpenDocument,
    dialect: Dialect,
    context: &Context,
    encoding: PositionEncoding,
) -> (PublishDiagnosticsParams, DocumentAnalysis) {
    let (found, analysis) = file_check::analyzed(path, &document.text, dialect, context);
    let lines = LineIndex::new(&document.text);
    let shown = found.iter().map(|diagnostic| editor_diagnostic(diagnostic, &lines, encoding)).collect();
    let analysis =
        DocumentAnalysis { uri: document.uri.clone(), path: path.to_path_buf(), text: document.text.clone(), analysis };
    (publication(document, shown), analysis)
}

fn publication(document: &OpenDocument, diagnostics: Vec<EditorDiagnostic>) -> PublishDiagnosticsParams {
    PublishDiagnosticsParams { uri: document.uri.clone(), diagnostics, version: Some(document.version) }
}

fn editor_diagnostic(diagnostic: &Diagnostic, lines: &LineIndex, encoding: PositionEncoding) -> EditorDiagnostic {
    let severity = match diagnostic.severity {
        Severity::Error => DiagnosticSeverity::ERROR,
        Severity::Warning => DiagnosticSeverity::WARNING,
    };
    EditorDiagnostic {
        range: lines.range(diagnostic, encoding),
        severity: Some(severity),
        source: Some(DIAGNOSTIC_SOURCE.to_string()),
        message: diagnostic.message.clone(),
        ..EditorDiagnostic::default()
    }
}
