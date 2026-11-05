//! Hover and go-to-definition, answered from a document's analysis. A name
//! imported from a module leads to where the module declares the export,
//! read with its unsaved text when it is open.

use lsp_types::{Hover, HoverContents, Location, MarkupContent, MarkupKind, Position};
use rowan::{TextRange, TextSize};
use ths_check::{ImportOrigin, TypeTarget, export_definition};
use ths_project::resolve_import;

use super::document_analyses::DocumentAnalysis;
use super::document_paths::file_uri;
use super::hover_text::hover_markdown;
use super::positions::{LineIndex, PositionEncoding};
use crate::source_files::SourceFiles;

pub fn hover(document: &DocumentAnalysis, position: Position, encoding: PositionEncoding) -> Option<Hover> {
    let lines = LineIndex::new(&document.text);
    let reference = document.analysis.reference_at(lines.offset(position, encoding))?;
    let symbol = document.analysis.symbol_of(reference);
    Some(Hover {
        contents: HoverContents::Markup(MarkupContent {
            kind: MarkupKind::Markdown,
            value: hover_markdown(reference, symbol),
        }),
        range: Some(lines.text_range(reference.range, encoding)),
    })
}

pub fn definition(
    document: &DocumentAnalysis,
    position: Position,
    encoding: PositionEncoding,
    sources: &dyn SourceFiles,
) -> Option<Location> {
    let lines = LineIndex::new(&document.text);
    let offset = lines.offset(position, encoding);
    let Some(reference) = document.analysis.reference_at(offset) else {
        return type_definition(document, offset, encoding, sources);
    };
    let symbol = document.analysis.symbol_of(reference)?;
    if let Some(origin) = &symbol.import {
        return imported_definition(document, origin, encoding, sources);
    }
    let range = lines.text_range(symbol.definition?, encoding);
    Some(Location { uri: document.uri.clone(), range })
}

/// Where the type name at `offset` is declared: in this file, or in the
/// module it is imported from.
fn type_definition(
    document: &DocumentAnalysis,
    offset: TextSize,
    encoding: PositionEncoding,
    sources: &dyn SourceFiles,
) -> Option<Location> {
    match &document.analysis.type_reference_at(offset)?.target {
        TypeTarget::InFile(range) => {
            let range = LineIndex::new(&document.text).text_range(*range, encoding);
            Some(Location { uri: document.uri.clone(), range })
        }
        TypeTarget::Imported(origin) => imported_definition(document, origin, encoding, sources),
    }
}

/// Where the module an import names declares the export it imports.
fn imported_definition(
    document: &DocumentAnalysis,
    origin: &ImportOrigin,
    encoding: PositionEncoding,
    sources: &dyn SourceFiles,
) -> Option<Location> {
    let module = resolve_import(&document.path, &origin.module_path).ok()?;
    let source = sources.read(&module).ok()?;
    // `* as name` is the whole module: its start.
    let range = match &origin.exported {
        Some(exported) => export_definition(&source, exported)?,
        None => TextRange::empty(TextSize::from(0)),
    };
    let range = LineIndex::new(&source).text_range(range, encoding);
    Some(Location { uri: file_uri(&module)?, range })
}
