use rowan::TextRange;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    Error,
    Warning,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnostic {
    pub severity: Severity,
    pub range: TextRange,
    pub message: String,
}

/// Where diagnostics accumulate during a check.
#[derive(Debug, Default)]
pub(crate) struct DiagnosticSink {
    pub(crate) diagnostics: Vec<Diagnostic>,
    /// Keep only rejections, dropping type errors and warnings.
    pub(crate) only_rejections: bool,
}

impl DiagnosticSink {
    pub(crate) fn only_rejections() -> Self {
        Self { only_rejections: true, ..Self::default() }
    }

    /// An error the TradeServer itself reports, such as a syntax error or
    /// a read of a name nothing defines: the script cannot be used at all.
    pub(crate) fn rejection(&mut self, range: TextRange, message: String) {
        self.add(Diagnostic { severity: Severity::Error, range, message });
    }

    pub(crate) fn error(&mut self, range: TextRange, message: String) {
        if self.only_rejections {
            return;
        }
        self.add(Diagnostic { severity: Severity::Error, range, message });
    }

    pub(crate) fn warning(&mut self, range: TextRange, message: String) {
        if self.only_rejections {
            return;
        }
        self.add(Diagnostic { severity: Severity::Warning, range, message });
    }

    /// Type syntax may be resolved more than once (a local function's
    /// signature, before and while its body is checked), so a diagnostic
    /// already reported is not reported again.
    fn add(&mut self, diagnostic: Diagnostic) {
        if !self.diagnostics.contains(&diagnostic) {
            self.diagnostics.push(diagnostic);
        }
    }
}
