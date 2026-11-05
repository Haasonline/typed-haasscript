//! What can be written where the cursor is: after a `.`, the fields and
//! methods of the value before it, and after a `:`, its methods; inside a comment or string, nothing;
//! anywhere else, the names in scope. The editor narrows the list to what
//! has been typed.

use lsp_types::{CompletionItem, CompletionItemKind, Documentation, MarkupContent, MarkupKind, Position};
use ths_check::{AnalyzedSymbol, Member};

use super::document_analyses::DocumentAnalysis;
use super::positions::{LineIndex, PositionEncoding};

/// What makes the editor ask for completions without being told to.
pub const TRIGGER_CHARACTERS: [&str; 2] = [".", ":"];

pub fn completion(document: &DocumentAnalysis, position: Position, encoding: PositionEncoding) -> Vec<CompletionItem> {
    let offset = LineIndex::new(&document.text).offset(position, encoding);
    if document.analysis.is_in_comment_or_string(offset) {
        return Vec::new();
    }
    match document.analysis.member_site_at(offset) {
        Some(site) => site.offered().map(member_item).collect(),
        None => document.analysis.names_in_scope(offset).into_iter().map(symbol_item).collect(),
    }
}

fn symbol_item(symbol: &AnalyzedSymbol) -> CompletionItem {
    let kind = if symbol.is_function() { CompletionItemKind::FUNCTION } else { CompletionItemKind::VARIABLE };
    CompletionItem {
        label: symbol.name.clone(),
        kind: Some(kind),
        detail: Some(symbol.type_text()),
        documentation: symbol.documentation.clone().map(|documentation| {
            Documentation::MarkupContent(MarkupContent { kind: MarkupKind::Markdown, value: documentation })
        }),
        ..CompletionItem::default()
    }
}

fn member_item(member: &Member) -> CompletionItem {
    let kind = if member.is_function() { CompletionItemKind::METHOD } else { CompletionItemKind::FIELD };
    let optional = if member.optional { "?" } else { "" };
    CompletionItem {
        label: member.name.clone(),
        kind: Some(kind),
        detail: Some(format!("{}{optional}: {}", member.name, member.type_text())),
        ..CompletionItem::default()
    }
}
