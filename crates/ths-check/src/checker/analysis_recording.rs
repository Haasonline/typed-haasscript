//! Records what `FileAnalysis` holds while the file is checked, for an
//! editor: only when asked, so `ths check` does not pay for it. The checker
//! resolves every name already; this notes what it resolved.

use std::collections::HashMap;

use rowan::{TextRange, TextSize};
use ths_syntax::{SyntaxKind, SyntaxNode, SyntaxToken};

use super::Checker;
use crate::analysis::{
    AnalyzedSymbol, FileAnalysis, ImportOrigin, Member, MemberAccess, MemberSite, Reference, ReferenceTarget,
    SymbolKind,
};
use crate::documentation::leading_comment;
use crate::scopes::SymbolId;
use crate::types::Type;
use crate::visibility::{BindingSite, visible_range};

#[derive(Debug, Default)]
pub(crate) struct AnalysisRecorder {
    enabled: bool,
    definitions: HashMap<SymbolId, Definition>,
    references: HashMap<TextRange, RecordedReference>,
    member_sites: HashMap<TextSize, RecordedMemberSite>,
}

#[derive(Debug)]
struct Definition {
    name: String,
    kind: SymbolKind,
    definition: Option<TextRange>,
    visible: Option<TextRange>,
    import: Option<ImportOrigin>,
    documentation: Option<String>,
}

#[derive(Debug)]
enum RecordedTarget {
    Symbol(SymbolId),
    Field(String),
}

#[derive(Debug)]
struct RecordedReference {
    target: RecordedTarget,
    /// `None`: the symbol's declared type once checking ends, for the name
    /// at a declaration, whose type can still be inferred later.
    value: Option<Type>,
}

#[derive(Debug)]
struct RecordedMemberSite {
    access: MemberAccess,
    name: Option<TextRange>,
    owner: Type,
}

/// How a name came to be declared.
pub(super) struct Declared<'syntax> {
    pub(super) kind: SymbolKind,
    /// The name in the declaration; `None` for one written nowhere (`self`).
    pub(super) name: Option<&'syntax SyntaxToken>,
    pub(super) visible: Option<TextRange>,
    /// The statement whose leading comment documents it.
    pub(super) statement: Option<&'syntax SyntaxNode>,
    pub(super) import: Option<ImportOrigin>,
}

impl AnalysisRecorder {
    pub(super) fn enable(&mut self) {
        self.enabled = true;
    }
}

impl Checker<'_> {
    pub(super) fn record_definition(&mut self, id: SymbolId, name: &str, declared: Declared) {
        let recorder = &mut self.analysis;
        if !recorder.enabled {
            return;
        }
        let range = declared.name.map(SyntaxToken::text_range);
        if let Some(range) = range {
            recorder.references.insert(range, RecordedReference { target: RecordedTarget::Symbol(id), value: None });
        }
        recorder.definitions.insert(
            id,
            Definition {
                name: name.to_string(),
                kind: declared.kind,
                definition: range,
                visible: declared.visible,
                import: declared.import,
                documentation: declared.statement.and_then(leading_comment),
            },
        );
    }

    /// A local declared at `name`, documented by `statement`'s comment.
    pub(super) fn record_local(&mut self, id: SymbolId, name: &SyntaxToken, site: BindingSite, statement: &SyntaxNode) {
        let declared = Declared {
            kind: SymbolKind::Local,
            name: Some(name),
            visible: visible_range(name, site),
            statement: Some(statement),
            import: None,
        };
        self.record_definition(id, name.text(), declared);
    }

    /// A global the script creates at `name`, visible in the whole file.
    pub(super) fn record_global(&mut self, id: SymbolId, name: &SyntaxToken, statement: &SyntaxNode) {
        let declared = Declared {
            kind: SymbolKind::Global,
            name: Some(name),
            visible: None,
            statement: Some(statement),
            import: None,
        };
        self.record_definition(id, name.text(), declared);
    }

    /// A read of, or an assignment to, the variable `id` at `name`.
    pub(super) fn record_read(&mut self, name: &SyntaxToken, id: SymbolId, value: &Type) {
        self.record_reference(name.text_range(), RecordedTarget::Symbol(id), value);
    }

    /// The field or method `name` of a table, holding `value`.
    pub(super) fn record_field(&mut self, name: &SyntaxToken, value: &Type) {
        self.record_reference(name.text_range(), RecordedTarget::Field(name.text().to_string()), value);
    }

    /// A `.` or `:` after a value of type `owner`, with the name after it.
    pub(super) fn record_member_site(&mut self, accessor: &SyntaxToken, name: Option<&SyntaxToken>, owner: &Type) {
        let recorder = &mut self.analysis;
        if !recorder.enabled {
            return;
        }
        let after = accessor.text_range().end();
        let access = if accessor.kind() == SyntaxKind::Colon { MemberAccess::Method } else { MemberAccess::Field };
        let owner = match recorder.member_sites.remove(&after) {
            Some(earlier) => Type::union([earlier.owner, owner.clone()]),
            None => owner.clone(),
        };
        recorder
            .member_sites
            .insert(after, RecordedMemberSite { access, name: name.map(SyntaxToken::text_range), owner });
    }

    /// A name checked more than once, as in a loop, keeps every type it had.
    fn record_reference(&mut self, range: TextRange, target: RecordedTarget, value: &Type) {
        let recorder = &mut self.analysis;
        if !recorder.enabled {
            return;
        }
        let value = match recorder.references.remove(&range) {
            Some(RecordedReference { value: Some(earlier), .. }) => Type::union([earlier, value.clone()]),
            Some(RecordedReference { value: None, target }) => {
                // The name at its own declaration stays the declaration.
                recorder.references.insert(range, RecordedReference { target, value: None });
                return;
            }
            None => value.clone(),
        };
        recorder.references.insert(range, RecordedReference { target, value: Some(value) });
    }

    /// The analysis, once the whole file is checked; `None` when it was not
    /// asked for.
    pub(super) fn finish_analysis(&mut self) -> Option<FileAnalysis> {
        let recorder = std::mem::take(&mut self.analysis);
        if !recorder.enabled {
            return None;
        }
        let mut ids: Vec<SymbolId> = recorder.definitions.keys().copied().collect();
        ids.sort_unstable();
        let index_of: HashMap<SymbolId, usize> = ids.iter().enumerate().map(|(index, id)| (*id, index)).collect();
        let symbols = self.analyzed_symbols(&ids, recorder.definitions);
        let references = self.analyzed_references(recorder.references, &index_of);
        let member_sites = self.analyzed_member_sites(recorder.member_sites);
        Some(FileAnalysis { symbols, references, member_sites, comment_and_string_interiors: Vec::new() })
    }

    /// The symbol of each of `ids`, in that order, with its declared type
    /// once checking ended.
    fn analyzed_symbols(
        &self,
        ids: &[SymbolId],
        mut definitions: HashMap<SymbolId, Definition>,
    ) -> Vec<AnalyzedSymbol> {
        ids.iter()
            .map(|id| {
                let definition = definitions.remove(id).expect("every id is defined");
                AnalyzedSymbol {
                    name: definition.name,
                    kind: definition.kind,
                    definition: definition.definition,
                    visible: definition.visible,
                    import: definition.import,
                    documentation: definition.documentation,
                    declared: self.scopes.declared_type(*id).clone(),
                }
            })
            .collect()
    }

    /// Every recorded reference, in source order, pointing at symbols by
    /// their position in `index_of`.
    fn analyzed_references(
        &self,
        recorded: HashMap<TextRange, RecordedReference>,
        index_of: &HashMap<SymbolId, usize>,
    ) -> Vec<Reference> {
        let mut references: Vec<Reference> = recorded
            .into_iter()
            .filter_map(|(range, recorded)| {
                let (target, declared) = match recorded.target {
                    RecordedTarget::Symbol(id) => {
                        (ReferenceTarget::Symbol(*index_of.get(&id)?), Some(self.scopes.declared_type(id).clone()))
                    }
                    RecordedTarget::Field(name) => (ReferenceTarget::Field(name), None),
                };
                let value = recorded.value.or(declared).unwrap_or(Type::Any);
                Some(Reference { range, target, value })
            })
            .collect();
        references.sort_by_key(|reference| reference.range.start());
        references
    }

    /// Every recorded member site, in source order, with the members its
    /// owner offers.
    fn analyzed_member_sites(&self, recorded: HashMap<TextSize, RecordedMemberSite>) -> Vec<MemberSite> {
        let mut member_sites: Vec<MemberSite> = recorded
            .into_iter()
            .map(|(after, site)| MemberSite {
                after,
                access: site.access,
                name: site.name,
                members: self.members_of(&site.owner),
            })
            .collect();
        member_sites.sort_by_key(|site| site.after);
        member_sites
    }

    /// The fields and methods of a value of type `owner`, in name order;
    /// for a union, those of any of its tables.
    fn members_of(&self, owner: &Type) -> Vec<Member> {
        let mut members: HashMap<String, Member> = HashMap::new();
        for part in self.expand(&owner.without_nil()).members() {
            if let Type::Object(object) = self.expand(part) {
                for (name, property) in &object.properties {
                    members.entry(name.clone()).or_insert_with(|| Member {
                        name: name.clone(),
                        optional: property.optional,
                        value: property.value.clone(),
                    });
                }
            }
        }
        let mut members: Vec<Member> = members.into_values().collect();
        members.sort_by(|left, right| left.name.cmp(&right.name));
        members
    }
}
