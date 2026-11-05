//! The analysis of each open document from the latest check, kept with
//! the text it was made from, so positions are read against that text.

use std::collections::HashMap;
use std::path::PathBuf;

use lsp_types::Uri;
use ths_check::FileAnalysis;

#[derive(Debug)]
pub struct DocumentAnalysis {
    pub uri: Uri,
    pub path: PathBuf,
    pub text: String,
    pub analysis: FileAnalysis,
}

#[derive(Debug, Default)]
pub struct DocumentAnalyses {
    /// By the URI as the editor wrote it.
    by_uri: HashMap<String, DocumentAnalysis>,
}

impl DocumentAnalyses {
    /// Replaces every analysis with those of the latest check.
    pub fn replace_all(&mut self, analyses: Vec<DocumentAnalysis>) {
        self.by_uri = analyses.into_iter().map(|analysis| (analysis.uri.as_str().to_string(), analysis)).collect();
    }

    pub fn get(&self, uri: &Uri) -> Option<&DocumentAnalysis> {
        self.by_uri.get(uri.as_str())
    }
}
