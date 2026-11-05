//! What `analyze_module` records for an editor.

use rowan::TextSize;
use ths_syntax::Dialect;

use crate::{
    CheckOptions, Declarations, FileAnalysis, ImportedModules, ModuleInterface, SymbolKind, TypeTarget, analyze_module,
    check_module,
};

fn analyzed(source: &str) -> FileAnalysis {
    analyze_module(
        source,
        Dialect::Typed,
        &Declarations::built_in(),
        CheckOptions::default(),
        &ImportedModules::default(),
        "",
    )
    .analysis
}

/// The offset of the `occurrence`th (from 1) `text` in `source`.
fn offset_of(source: &str, text: &str, occurrence: usize) -> TextSize {
    let mut from = 0;
    let mut found = 0;
    loop {
        let index = from + source[from..].find(text).expect("the text is in the source");
        found += 1;
        if found == occurrence {
            return TextSize::from(u32::try_from(index).unwrap());
        }
        from = index + 1;
    }
}

/// The type at a name, and where its symbol is defined.
fn hover(analysis: &FileAnalysis, offset: TextSize) -> (String, Option<TextSize>) {
    let reference = analysis.reference_at(offset).expect("a name is there");
    let definition = analysis.symbol_of(reference).and_then(|symbol| symbol.definition).map(|range| range.start());
    (reference.type_text(), definition)
}

#[test]
fn a_read_has_its_narrowed_type_and_leads_to_its_declaration() {
    let source = "local function show(price: number | nil)\n    if price then\n        Log(price)\n    end\nend";
    let analysis = analyzed(source);
    let (shown, definition) = hover(&analysis, offset_of(source, "price", 3));
    assert_eq!(shown, "number");
    assert_eq!(definition, Some(offset_of(source, "price", 1)));
}

#[test]
fn a_declaration_shows_the_type_inferred_for_it() {
    let source = "local function double(value: number)\n    return value * 2\nend";
    let analysis = analyzed(source);
    let (shown, _) = hover(&analysis, offset_of(source, "double", 1));
    assert_eq!(shown, "(value: number) => number");
}

#[test]
fn a_field_has_its_member_type() {
    let source = "local quote = { bid = 1, label = \"x\" }\nLog(quote.label)";
    let analysis = analyzed(source);
    let reference = analysis.reference_at(offset_of(source, "label", 2)).unwrap();
    assert_eq!(reference.type_text(), "string");
}

#[test]
fn a_built_in_command_has_its_documentation() {
    let source = "local prices = ClosePrices()";
    let analysis = analyzed(source);
    let reference = analysis.reference_at(offset_of(source, "ClosePrices", 1)).unwrap();
    let symbol = analysis.symbol_of(reference).unwrap();
    assert_eq!(symbol.kind, SymbolKind::Declared);
    assert_eq!(symbol.definition, None);
    assert!(symbol.documentation.as_deref().is_some_and(|text| !text.is_empty()), "{symbol:?}");
}

#[test]
fn a_comment_above_a_local_documents_it_but_one_trailing_code_does_not() {
    let source = "local first = 1 -- a note on first\nlocal second = 2\n-- The upper bound.\nlocal limit = 3";
    let analysis = analyzed(source);
    let documentation = |name: &str| {
        let reference = analysis.reference_at(offset_of(source, name, 1)).unwrap();
        analysis.symbol_of(reference).unwrap().documentation.clone()
    };
    assert_eq!(documentation("second"), None);
    assert_eq!(documentation("limit").as_deref(), Some("The upper bound."));
}

#[test]
fn names_in_scope_follow_blocks_and_hiding() {
    let source = "local outer = 1\ndo\n    local inner = \"x\"\n    local outer = \"y\"\n    \nend\n";
    let analysis = analyzed(source);
    let inside = offset_of(source, "    \nend", 1) + TextSize::from(4);
    let names: Vec<(String, String)> = analysis
        .names_in_scope(inside)
        .into_iter()
        .filter(|symbol| symbol.kind != SymbolKind::Declared)
        .map(|symbol| (symbol.name.clone(), symbol.type_text()))
        .collect();
    assert_eq!(names, [("inner".to_string(), "string".to_string()), ("outer".to_string(), "string".to_string())]);
    let after = TextSize::of(source);
    let outside: Vec<String> = analysis
        .names_in_scope(after)
        .into_iter()
        .filter(|symbol| symbol.kind != SymbolKind::Declared)
        .map(|symbol| symbol.name.clone())
        .collect();
    assert_eq!(outside, ["outer"]);
}

#[test]
fn a_parameter_is_in_scope_only_in_its_function() {
    let source = "local function f(amount: number)\n    \nend\n";
    let analysis = analyzed(source);
    let in_scope = |offset: TextSize| {
        analysis
            .names_in_scope(offset)
            .iter()
            .any(|symbol| symbol.name == "amount" && symbol.kind == SymbolKind::Parameter)
    };
    assert!(in_scope(offset_of(source, "    \nend", 1) + TextSize::from(4)));
    assert!(!in_scope(TextSize::of(source)));
}

#[test]
fn an_unfinished_field_offers_the_owners_fields() {
    let source = "local quote = { bid = 1, ask = 2 }\nlocal spread = quote.";
    let analysis = analyzed(source);
    let site = analysis.member_site_at(TextSize::of(source)).expect("a member site at the end");
    let names: Vec<&str> = site.members.iter().map(|member| member.name.as_str()).collect();
    assert_eq!(names, ["ask", "bid"]);
}

#[test]
fn a_method_call_offers_the_owners_methods_while_its_name_is_written() {
    let source = "local account = { Balance = function(self): number return 1 end }\nLog(account:Bal())";
    let analysis = analyzed(source);
    let site = analysis.member_site_at(offset_of(source, "Bal()", 1) + TextSize::from(2)).unwrap();
    assert!(site.members.iter().any(|member| member.name == "Balance" && member.is_function()));
}

#[test]
fn an_import_records_where_it_comes_from() {
    let module = check_module(
        "export local bid = 1",
        Dialect::Typed,
        &Declarations::built_in(),
        CheckOptions::default(),
        &ImportedModules::default(),
        "quotes",
    );
    let interface: ModuleInterface = module.interface.unwrap();
    let mut modules = ImportedModules::default();
    modules.insert("./quotes", &interface);
    let source = "import { bid } from \"./quotes\"\nLog(bid)";
    let analysis =
        analyze_module(source, Dialect::Typed, &Declarations::built_in(), CheckOptions::default(), &modules, "")
            .analysis;
    let reference = analysis.reference_at(offset_of(source, "bid", 2)).unwrap();
    let symbol = analysis.symbol_of(reference).unwrap();
    assert_eq!(symbol.kind, SymbolKind::Import);
    let origin = symbol.import.as_ref().unwrap();
    assert_eq!((origin.module_path.as_str(), origin.exported.as_deref()), ("./quotes", Some("bid")));
    assert_eq!(reference.type_text(), "number");
}

#[test]
fn checking_without_analysis_reports_the_same_diagnostics() {
    let source = "local a: string = 1\nlocal b = a.";
    let declarations = Declarations::built_in();
    let options = CheckOptions::default();
    let modules = ImportedModules::default();
    let plain = check_module(source, Dialect::Typed, &declarations, options, &modules, "").diagnostics;
    let analyzed = analyze_module(source, Dialect::Typed, &declarations, options, &modules, "").checked.diagnostics;
    assert_eq!(plain, analyzed);
}

#[test]
fn an_export_is_found_at_its_declaration_even_when_listed_under_another_name() {
    let source = "local function average(values: number[]): number\n    return values[1]\nend\nexport local limit = 3\nexport { average as mean }";
    assert_eq!(
        crate::export_definition(source, "limit").map(|range| range.start()),
        Some(offset_of(source, "limit", 1))
    );
    assert_eq!(
        crate::export_definition(source, "mean").map(|range| range.start()),
        Some(offset_of(source, "average", 1))
    );
    assert_eq!(crate::export_definition(source, "missing"), None);
}

fn inside(source: &str, offset: u32) -> bool {
    analyzed(source).is_in_comment_or_string(TextSize::from(offset))
}

#[test]
fn a_closed_string_holds_the_cursor_only_between_its_quotes() {
    let source = "Log(\"ab\")";
    assert!(!inside(source, 4));
    assert!(inside(source, 5));
    assert!(inside(source, 7));
    assert!(!inside(source, 8));
}

#[test]
fn a_comment_or_string_still_open_holds_the_cursor_at_its_end() {
    assert!(inside("Log(1) -- note", 14));
    assert!(inside("local a = \"unfinished", 21));
    assert!(inside("local a = \"escaped\\\"", 20));
    assert!(inside("--[==[ open ]]", 14));
    assert!(!inside("--[==[ closed ]==]", 18));
    assert!(!inside("local a = [[x]]", 15));
}

#[test]
fn code_is_not_inside_a_comment_or_string() {
    assert!(!inside("local period = 14", 6));
}

#[test]
fn a_colon_offers_only_what_can_be_called_and_a_dot_offers_everything() {
    let source =
        "local account = { cash = 1, Balance = function(self): number return 1 end }\nLog(account:)\nLog(account.)";
    let analysis = analyzed(source);
    let offered = |text: &str| -> Vec<String> {
        let site = analysis.member_site_at(offset_of(source, text, 1) + TextSize::of(text)).unwrap();
        site.offered().map(|member| member.name.clone()).collect()
    };
    assert_eq!(offered("account:"), ["Balance"]);
    assert_eq!(offered("account."), ["Balance", "cash"]);
}

/// Where the type name at `offset` is declared in the file.
fn type_definition(analysis: &FileAnalysis, offset: TextSize) -> Option<TextSize> {
    match &analysis.type_reference_at(offset)?.target {
        TypeTarget::InFile(range) => Some(range.start()),
        TypeTarget::Imported(_) => None,
    }
}

#[test]
fn a_type_parameter_name_leads_to_the_innermost_parameter() {
    let source = "type Box<T> = { value: T }\nlocal function first<T>(items: T[]): T\n    return items[1]\nend";
    let analysis = analyzed(source);
    assert_eq!(type_definition(&analysis, offset_of(source, "T", 2)), Some(offset_of(source, "T", 1)));
    assert_eq!(type_definition(&analysis, offset_of(source, "T", 4)), Some(offset_of(source, "T", 3)));
    assert_eq!(type_definition(&analysis, offset_of(source, "T", 5)), Some(offset_of(source, "T", 3)));
}

#[test]
fn a_built_in_type_name_has_no_declaration_in_the_file() {
    let source = "local price: number = 1";
    let analysis = analyzed(source);
    assert!(analysis.type_reference_at(offset_of(source, "number", 1)).is_none());
}
