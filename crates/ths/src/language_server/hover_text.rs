//! What hovering over a name shows: how it was declared and its type
//! where it is written, as a line of code, then its documentation.

use ths_check::{AnalyzedSymbol, Reference, ReferenceTarget, SymbolKind};

/// The language the code line is marked as, so the editor highlights it.
const CODE_LANGUAGE: &str = "typed-haasscript";
const SEPARATOR: &str = "\n\n---\n\n";

pub fn hover_markdown(reference: &Reference, symbol: Option<&AnalyzedSymbol>) -> String {
    let declaration = match (&reference.target, symbol) {
        (ReferenceTarget::Symbol(_), Some(symbol)) => {
            format!("{}{}: {}", kind_prefix(symbol.kind), symbol.name, reference.type_text())
        }
        (ReferenceTarget::Field(name), _) => format!("(field) {name}: {}", reference.type_text()),
        (ReferenceTarget::Symbol(_), None) => reference.type_text(),
    };
    let code = format!("```{CODE_LANGUAGE}\n{declaration}\n```");
    match symbol.and_then(|symbol| symbol.documentation.as_deref()) {
        Some(documentation) => format!("{code}{SEPARATOR}{documentation}"),
        None => code,
    }
}

fn kind_prefix(kind: SymbolKind) -> &'static str {
    match kind {
        SymbolKind::Local => "local ",
        SymbolKind::Parameter => "(parameter) ",
        SymbolKind::Global => "(global) ",
        SymbolKind::Declared => "declare ",
        SymbolKind::Import => "(import) ",
    }
}
