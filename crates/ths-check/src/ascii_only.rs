//! The TradeServer deletes every non-ASCII character when a script is
//! saved (verified: `Log("é")` saves as `Log("")`), so each run of them is
//! reported where it can still be fixed.

use rowan::{TextRange, TextSize};

use crate::diagnostic::DiagnosticSink;

pub(crate) fn report_non_ascii(source: &str, sink: &mut DiagnosticSink) {
    let mut run_start: Option<usize> = None;
    for (offset, character) in source.char_indices().chain(std::iter::once((source.len(), ' '))) {
        match (character.is_ascii(), run_start) {
            (false, None) => run_start = Some(offset),
            (true, Some(start)) => {
                let range = TextRange::new(text_size(start), text_size(offset));
                sink.warning(
                    range,
                    "The TradeServer deletes non-ASCII characters when the script is saved.".to_string(),
                );
                run_start = None;
            }
            _ => {}
        }
    }
}

fn text_size(offset: usize) -> TextSize {
    TextSize::try_from(offset).expect("source larger than 4 GiB")
}
