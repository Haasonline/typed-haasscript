//! Modules: the rules for imports and exports in a file, its module outline, and what an import sees of the module it names.

use ths_syntax::Dialect;

use crate::test_support::errors;
use crate::{CheckOptions, Declarations, ImportedModules, ModuleInterface, Severity, check_module};

const PRICES_PATH: &str = "./prices";

const PRICES: &str = "\
export interface Quote { bid: number, ask: number }
export type Side = \"buy\" | \"sell\"
export local spreadLimit: number = 2
export local function spread(quote: Quote): number
    return quote.ask - quote.bid
end
";

fn interface_of(source: &str, name: &str) -> ModuleInterface {
    let checked = check_module(
        source,
        Dialect::Typed,
        &Declarations::default(),
        CheckOptions::default(),
        &ImportedModules::default(),
        name,
    );
    assert_eq!(
        checked.diagnostics.iter().map(|diagnostic| diagnostic.message.clone()).collect::<Vec<_>>(),
        Vec::<String>::new()
    );
    checked.interface.expect("a file with exports is a module")
}

/// The errors in `source`, which imports `PRICES` as `./prices`.
fn errors_importing_prices(source: &str) -> Vec<String> {
    let prices = interface_of(PRICES, "prices");
    let mut modules = ImportedModules::default();
    modules.insert(PRICES_PATH, &prices);
    check_module(source, Dialect::Typed, &Declarations::default(), CheckOptions::default(), &modules, "bot")
        .diagnostics
        .into_iter()
        .filter(|diagnostic| diagnostic.severity == Severity::Error)
        .map(|diagnostic| diagnostic.message)
        .collect()
}

#[test]
fn an_imported_value_has_its_exported_type() {
    assert_eq!(
        errors_importing_prices("import { spreadLimit } from \"./prices\"\nlocal text: string = spreadLimit"),
        ["Type 'number' is not assignable to type 'string'."]
    );
}

#[test]
fn an_imported_function_checks_its_arguments() {
    assert_eq!(
        errors_importing_prices("import { spread } from \"./prices\"\nLog(spread({ bid = 1 }))"),
        ["Property 'ask' is missing but required in type 'import(\"prices\").Quote'."]
    );
}

#[test]
fn an_imported_type_names_the_exported_declaration() {
    assert_eq!(
        errors_importing_prices(
            "import { type Quote as PriceQuote, spread } from \"./prices\"\nlocal quote: PriceQuote = { bid = 1, ask = 2 }\nLog(spread(quote))"
        ),
        Vec::<String>::new()
    );
    assert_eq!(
        errors_importing_prices("import { type Side } from \"./prices\"\nlocal side: Side = \"hold\""),
        ["Type '\"hold\"' is not assignable to type 'Side'."]
    );
}

#[test]
fn a_type_is_imported_with_type_and_a_value_without_it() {
    assert_eq!(
        errors_importing_prices("import { Quote, type spread } from \"./prices\""),
        ["'Quote' is a type: import it with 'type'.", "'spread' is a value: import it without 'type'."]
    );
}

#[test]
fn an_import_names_an_export_of_the_module() {
    assert_eq!(
        errors_importing_prices("import { missing } from \"./prices\"\nLog(missing)"),
        ["Module './prices' has no export 'missing'."]
    );
}

#[test]
fn a_namespace_import_holds_every_exported_value() {
    assert_eq!(
        errors_importing_prices("import * as prices from \"./prices\"\nLog(prices.spreadLimit)\nLog(prices.Quote)"),
        [
            "Property 'Quote' does not exist on type '{ spread: (quote: import(\"prices\").Quote) => number; spreadLimit: number }'."
        ]
    );
}

#[test]
fn an_import_is_read_only() {
    assert_eq!(
        errors_importing_prices("import { spreadLimit } from \"./prices\"\nspreadLimit = 3"),
        ["Cannot assign to 'spreadLimit': an import is read-only."]
    );
}

#[test]
fn an_unused_import_is_an_error() {
    assert_eq!(
        errors_importing_prices("import { spreadLimit, type Side } from \"./prices\""),
        ["'spreadLimit' is imported but never used.", "'Side' is imported but never used."]
    );
}

#[test]
fn a_module_does_not_reassign_an_export() {
    let checked = check_module(
        "export local count = 0\nexport local function bump()\n    count = count + 1\nend",
        Dialect::Typed,
        &Declarations::default(),
        CheckOptions::default(),
        &ImportedModules::default(),
        "counter",
    );
    let messages: Vec<String> = checked.diagnostics.into_iter().map(|diagnostic| diagnostic.message).collect();
    assert_eq!(
        messages,
        [
            "Cannot assign to 'count': it is exported, and importing files would not see the new value. Share changing state through a table's fields or a function."
        ]
    );
}

#[test]
fn a_module_may_change_an_exported_table_field() {
    let checked = check_module(
        "export local state = { count = 0 }\nexport local function bump()\n    state.count = state.count + 1\nend",
        Dialect::Typed,
        &Declarations::default(),
        CheckOptions::default(),
        &ImportedModules::default(),
        "counter",
    );
    assert!(checked.diagnostics.is_empty(), "{:?}", checked.diagnostics);
}

#[test]
fn a_module_passes_on_the_types_it_imports() {
    let prices = interface_of(PRICES, "prices");
    let mut for_quotes = ImportedModules::default();
    for_quotes.insert(PRICES_PATH, &prices);
    let quotes = check_module(
        "import { type Quote } from \"./prices\"\nexport local function latest(): Quote\n    return { bid = 1, ask = 2 }\nend",
        Dialect::Typed,
        &Declarations::default(),
        CheckOptions::default(),
        &for_quotes,
        "quotes",
    )
    .interface
    .expect("a module");
    let mut for_bot = ImportedModules::default();
    for_bot.insert("./quotes", &quotes);
    let messages: Vec<String> = check_module(
        "import { latest } from \"./quotes\"\nlocal bid: string = latest().bid",
        Dialect::Typed,
        &Declarations::default(),
        CheckOptions::default(),
        &for_bot,
        "bot",
    )
    .diagnostics
    .into_iter()
    .map(|diagnostic| diagnostic.message)
    .collect();
    assert_eq!(messages, ["Type 'number' is not assignable to type 'string'."]);
}

// Rules within one file

#[test]
fn an_exported_declaration_is_checked_as_any_other() {
    assert_eq!(
        errors("export local limit = 10\nexport type Label = string\nlocal text: Label = limit"),
        ["Type 'number' is not assignable to type 'Label'."]
    );
}

#[test]
fn imports_and_exports_only_at_the_top_level() {
    assert_eq!(
        errors("local function f()\n  export local inner = 1\nend\nexport local outer = 1"),
        ["Imports and exports are allowed only at the top level of a file."]
    );
}

#[test]
fn imports_come_before_every_other_statement() {
    assert_eq!(
        errors("import { a } from \"./a\"\nlocal b = 1\nimport { c } from \"./c\"\nLog(a .. b .. c)"),
        ["Imports must come before every other statement."]
    );
}

#[test]
fn a_module_holds_only_declarations_at_its_top_level() {
    assert_eq!(
        errors("export local limit = 10\nLog(limit)\nif limit then end"),
        [
            "A module may hold only declarations and imports at its top level: this statement would run on every tick of every script that imports it.",
            "A module may hold only declarations and imports at its top level: this statement would run on every tick of every script that imports it.",
        ]
    );
}

#[test]
fn a_module_declaration_may_call_a_command() {
    assert_eq!(
        errors("export local close = ClosePrices()\nlocal function helper() Log(close) end"),
        Vec::<String>::new()
    );
}

#[test]
fn a_trade_script_may_run_code_at_its_top_level() {
    assert_eq!(errors("import { limit } from \"./limits\"\nLog(limit)"), Vec::<String>::new());
}

#[test]
fn a_module_may_not_define_a_command_interface() {
    assert_eq!(
        errors("export local function setUp() DefineParameter(NumberType, 'a', 'x', true, 1) end"),
        ["A module may not call DefineParameter: a custom command's interface belongs in the command's own file."]
    );
}

#[test]
fn an_import_may_not_share_a_name_with_a_declaration() {
    assert_eq!(
        errors("import { limit } from \"./limits\"\nlocal limit = 5"),
        ["Import 'limit' has the same name as a declaration in this file."]
    );
}

#[test]
fn a_name_is_imported_once() {
    assert_eq!(
        errors("import { limit } from \"./limits\"\nimport { other as limit } from \"./others\"\nLog(limit)"),
        ["Duplicate import 'limit'."]
    );
}

#[test]
fn an_export_list_names_a_top_level_declaration() {
    assert_eq!(
        errors("import { limit } from \"./limits\"\nlocal helper = 1\nexport { helper as assist, missing, limit }"),
        [
            "Cannot export 'missing': nothing at the top level of this file has that name.",
            "Cannot export the import 'limit': re-exports are not supported.",
        ]
    );
}

#[test]
fn a_name_is_exported_once() {
    assert_eq!(errors("export local helper = 1\nexport { helper }"), ["Duplicate export 'helper'."]);
}

#[test]
fn a_module_outline_lists_its_imports_and_role() {
    let outline = crate::module_outline("import { a } from \"./a\"\nimport * as b from \"../b\"\nexport local c = a");
    assert!(outline.is_module);
    let paths: Vec<&str> = outline.imports.iter().map(|import| import.reference.path.as_str()).collect();
    assert_eq!(paths, ["./a", "../b"]);
    assert!(!crate::module_outline("import { a } from \"./a\"\nLog(a)").is_module);
}

#[test]
fn a_module_outline_leaves_types_out_of_the_bundled_values() {
    let outline = crate::module_outline(
        "import { type Quote, spread as gap } from \"./prices\"\nexport type Side = \"buy\"\nlocal limit = gap\nexport { limit as maximum }",
    );
    let imported: Vec<(&str, Option<&str>)> =
        outline.imports[0].values.iter().map(|value| (value.local.as_str(), value.exported.as_deref())).collect();
    assert_eq!(imported, [("gap", Some("spread"))]);
    let exported: Vec<(&str, &str)> =
        outline.exported_values.iter().map(|value| (value.exported.as_str(), value.local.as_str())).collect();
    assert_eq!(exported, [("maximum", "limit")]);
}
