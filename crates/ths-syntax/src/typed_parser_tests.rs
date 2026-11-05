use crate::test_support::outline;
use crate::{Dialect, parse};

/// Parses typed HaasScript, asserts it has no errors, and returns the
/// outline of the whole tree.
fn typed_outline(source: &str) -> String {
    let parsed = parse(source, Dialect::Typed);
    assert!(parsed.errors.is_empty(), "{source}\n{:?}", parsed.errors);
    outline(&parsed.syntax_node())
}

fn typed_errors(source: &str) -> Vec<String> {
    parse(source, Dialect::Typed).errors.into_iter().map(|error| error.message).collect()
}

fn plain_errors(source: &str) -> Vec<String> {
    parse(source, Dialect::Plain).errors.into_iter().map(|error| error.message).collect()
}

#[test]
fn tree_text_is_the_typed_source_exactly() {
    let source = "interface A { x: number }\nlocal a: A = { x = 1 } -- note\n";
    assert_eq!(parse(source, Dialect::Typed).syntax_node().to_string(), source);
}

#[test]
fn local_with_a_type_annotation() {
    let tree = typed_outline("local x: number = 1");
    assert!(
        tree.contains("NameBinding\n          \"x\"\n          TypeAnnotation\n            \":\"\n            NameType\n              \"number\""),
        "{tree}"
    );
}

#[test]
fn function_with_typed_parameters_and_return_type() {
    let tree = typed_outline("local function f(a: number, b?: string, ...: boolean): number return a end");
    assert!(tree.contains("\"?\""), "{tree}");
    assert!(tree.contains("ReturnTypeAnnotation"), "{tree}");
    assert_eq!(tree.matches(" TypeAnnotation\n").count(), 3, "{tree}");
}

#[test]
fn generic_function() {
    let tree = typed_outline("function identity<T extends number = number>(value: T): T return value end");
    assert!(tree.contains("TypeParameterList"), "{tree}");
    assert!(tree.contains("\"extends\""), "{tree}");
}

#[test]
fn typed_lambda_parameter_stops_at_the_closing_pipe() {
    let tree = typed_outline("f = |x: number| x * 2");
    assert!(tree.contains("BinaryExpression"), "{tree}");
    assert!(!tree.contains("UnionType"), "{tree}");
}

#[test]
fn lambda_parameter_union_needs_parentheses() {
    let tree = typed_outline("f = |x: (number | string)| x");
    assert!(tree.contains("ParenthesizedType\n"), "{tree}");
    assert!(tree.contains("UnionType"), "{tree}");
}

#[test]
fn typed_numeric_for() {
    let tree = typed_outline("for i: number = 1, 3 do end");
    assert!(tree.contains("NumericForStatement"), "{tree}");
    assert!(tree.contains("TypeAnnotation"), "{tree}");
}

#[test]
fn typed_generic_for() {
    let tree = typed_outline("for key: string, value: number in pairs(t) do end");
    assert!(tree.contains("GenericForStatement\n      \"for\"\n      NameList"), "{tree}");
}

#[test]
fn type_alias_with_a_leading_pipe_union() {
    let tree = typed_outline("type Side = | \"long\" | \"short\"");
    assert!(tree.contains("TypeAliasStatement"), "{tree}");
    assert_eq!(tree.matches("LiteralType").count(), 2, "{tree}");
}

#[test]
fn interface_with_every_member_kind() {
    let tree = typed_outline(
        "interface Bot<T> extends Base, Other<T> {\n  name: string\n  size?: number,\n  step(a: T): nil;\n  [key: string]: any\n}",
    );
    assert!(tree.contains("ExtendsClause"), "{tree}");
    assert!(tree.contains("PropertySignature"), "{tree}");
    assert!(tree.contains("MethodSignature"), "{tree}");
    assert!(tree.contains("IndexSignature"), "{tree}");
}

#[test]
fn intersection_binds_tighter_than_union() {
    let tree = typed_outline("type T = A | B & C");
    assert!(tree.contains("UnionType\n"), "{tree}");
    assert!(tree.contains("\"|\"\n        IntersectionType"), "{tree}");
}

#[test]
fn array_of_a_generic_type() {
    let tree = typed_outline("local x: Map<string, number[]>[] = {}");
    assert_eq!(tree.matches("ArrayType").count(), 2, "{tree}");
    assert!(tree.contains("TypeArgumentList"), "{tree}");
}

#[test]
fn greater_equals_after_type_arguments_is_split() {
    let tree = typed_outline("local x: Array<number>= {}");
    assert!(tree.contains("\">\""), "{tree}");
    assert!(tree.contains("\"=\""), "{tree}");
}

#[test]
fn function_type_is_told_apart_from_a_parenthesized_type() {
    let function = typed_outline("type F = (a: number) => string");
    assert!(function.contains("FunctionType"), "{function}");
    let parenthesized = typed_outline("type P = (number)");
    assert!(parenthesized.contains("ParenthesizedType"), "{parenthesized}");
}

#[test]
fn return_type_may_list_several_values() {
    let tree = typed_outline("local function f(): (number, string) return 1, 'a' end");
    assert!(
        tree.contains(
            "ReturnTypeAnnotation\n          \":\"\n          TypeList\n            \"(\"\n            NameType"
        ),
        "{tree}"
    );
}

#[test]
fn declare_type_declares_an_opaque_type() {
    let tree = typed_outline("declare type EnumSignal extends Enum");
    assert!(
        tree.contains(
            "DeclareTypeStatement\n      \"declare\"\n      \"type\"\n      \"EnumSignal\"\n      ExtendsClause"
        ),
        "{tree}"
    );
}

#[test]
fn declare_type_may_have_call_signatures() {
    let tree = typed_outline("declare type EnumColor extends Enum { (opacity: number): EnumColor }");
    assert!(tree.contains("ObjectType\n        \"{\"\n        CallSignature\n          ParameterList"), "{tree}");
}

#[test]
fn declare_type_with_call_signatures_parses_without_errors() {
    assert_eq!(
        typed_errors("declare type EnumColor extends Enum { (opacity: number): EnumColor }"),
        Vec::<String>::new()
    );
}

#[test]
fn call_signature_may_be_generic() {
    let tree = typed_outline("interface Factory { <T>(value: T): T }");
    assert!(tree.contains("CallSignature\n          TypeParameterList"), "{tree}");
}

#[test]
fn declare_type_followed_by_a_colon_is_a_variable_named_type() {
    let tree = typed_outline("declare type: number");
    assert!(tree.contains("DeclareVariableStatement"), "{tree}");
}

#[test]
fn return_type_list_may_end_with_rest_values() {
    let tree = typed_outline("local function f(): (number, ...string) end");
    assert!(tree.contains("TypeList\n            \"(\"\n            NameType\n              \"number\"\n            \",\"\n            VariadicType"), "{tree}");
}

#[test]
fn return_type_may_be_rest_values_alone() {
    let tree = typed_outline("type F = () => ...string");
    assert!(tree.contains("VariadicType\n          \"...\""), "{tree}");
}

#[test]
fn explicit_type_arguments_instantiate_the_callee() {
    let tree = typed_outline("f<<number, string>>(x)");
    assert!(tree.contains("CallExpression\n        InstantiationExpression\n          NameExpression"), "{tree}");
}

#[test]
fn explicit_type_arguments_may_follow_a_method_name() {
    let tree = typed_outline("t:get<<number>>('k')");
    assert!(tree.contains("\"get\"\n        TypeInstantiation"), "{tree}");
}

#[test]
fn explicit_type_arguments_may_end_with_nested_type_arguments() {
    let tree = typed_outline("f<<Box<number>>>(x)");
    assert!(tree.contains("TypeInstantiation"), "{tree}");
}

#[test]
fn generic_function_type() {
    let tree = typed_outline("type F = <T>(value: T) => T");
    assert!(tree.contains("FunctionType\n        TypeParameterList"), "{tree}");
}

#[test]
fn cast_binds_like_a_comparison() {
    let tree = typed_outline("x = a + b as number");
    assert!(tree.contains("CastExpression\n          BinaryExpression"), "{tree}");
}

#[test]
fn declare_function_and_variable() {
    let tree =
        typed_outline("declare function ClosePrices(interval?: number): number[]\ndeclare SignalLong: EnumSignal");
    assert!(tree.contains("DeclareFunctionStatement"), "{tree}");
    assert!(tree.contains("DeclareVariableStatement"), "{tree}");
}

#[test]
fn type_as_a_function_call_is_still_a_call() {
    let tree = typed_outline("type(x)");
    assert!(tree.contains("CallStatement"), "{tree}");
}

#[test]
fn as_is_an_ordinary_name_in_plain_dialect() {
    assert!(plain_errors("local y = x as(1)").is_empty());
}

#[test]
fn annotations_are_errors_in_plain_dialect() {
    assert_eq!(plain_errors("local x: number = 1"), ["type annotations are only allowed in .ths files"]);
}

#[test]
fn explicit_type_arguments_are_errors_in_plain_dialect() {
    assert_eq!(plain_errors("f<<number>>(x)"), ["type arguments are only allowed in .ths files"]);
}

#[test]
fn type_alias_is_not_a_declaration_in_plain_dialect() {
    assert!(!plain_errors("type X = number").is_empty());
}

#[test]
fn missing_type_is_reported() {
    assert_eq!(typed_errors("local x: = 1"), ["expected a type"]);
}

#[test]
fn interface_with_a_numeric_member_name() {
    let tree = typed_outline("interface Position { market: string; 1: string }");
    assert!(tree.contains("PropertySignature\n          \"1\""), "{tree}");
}

// Modules

#[test]
fn named_imports_with_renaming_and_a_type_only_name() {
    let tree = typed_outline("import { average, sum as total, type Prices } from \"./math/averages\"");
    assert!(tree.contains("ImportStatement"), "{tree}");
    assert_eq!(tree.matches("ImportSpecifier").count(), 3, "{tree}");
    assert!(tree.contains("ImportSpecifier\n        \"type\"\n        \"Prices\""), "{tree}");
    assert!(tree.contains("ImportSpecifier\n        \"sum\"\n        \"as\"\n        \"total\""), "{tree}");
    assert!(tree.contains("\"from\"\n      \"\\\"./math/averages\\\"\""), "{tree}");
}

#[test]
fn a_namespace_import() {
    let tree = typed_outline("import * as averages from \"./math/averages\"");
    assert!(tree.contains("NamespaceImport\n        \"*\"\n        \"as\"\n        \"averages\""), "{tree}");
}

#[test]
fn an_imported_name_called_type() {
    let tree = typed_outline("import { type as kind } from \"./kinds\"");
    assert!(tree.contains("ImportSpecifier\n        \"type\"\n        \"as\"\n        \"kind\""), "{tree}");
}

#[test]
fn exported_declarations() {
    let tree = typed_outline(
        "export local limit: number = 10\nexport local function average(a: number): number return a end\nexport type Prices = number[]\nexport interface Bar { close: number }",
    );
    assert_eq!(tree.matches("ExportDeclaration").count(), 4, "{tree}");
    assert!(tree.contains("ExportDeclaration\n      \"export\"\n      LocalStatement"), "{tree}");
    assert!(tree.contains("ExportDeclaration\n      \"export\"\n      LocalFunctionStatement"), "{tree}");
    assert!(tree.contains("ExportDeclaration\n      \"export\"\n      TypeAliasStatement"), "{tree}");
    assert!(tree.contains("ExportDeclaration\n      \"export\"\n      InterfaceStatement"), "{tree}");
}

#[test]
fn an_export_list() {
    let tree = typed_outline("local helper = 1\nexport { helper as assist, helper }");
    assert!(tree.contains("ExportListStatement"), "{tree}");
    assert_eq!(tree.matches("ExportSpecifier").count(), 2, "{tree}");
}

#[test]
fn an_import_without_from_is_reported() {
    assert_eq!(typed_errors("import { average } \"./averages\""), ["expected 'from'"]);
}

#[test]
fn import_and_export_stay_ordinary_names_elsewhere() {
    let tree = typed_outline("local import = 1\nexport = import + 1\nimport(2)");
    assert!(!tree.contains("ImportStatement"), "{tree}");
    assert!(!tree.contains("Export"), "{tree}");
}

#[test]
fn an_import_is_two_calls_in_plain_dialect() {
    // `import{average}` and `from"./averages"` are both Lua calls.
    assert_eq!(plain_errors("import { average } from \"./averages\""), Vec::<String>::new());
    let tree = outline(&parse("import { average } from \"./averages\"", Dialect::Plain).syntax_node());
    assert!(!tree.contains("ImportStatement"), "{tree}");
    assert_eq!(tree.matches("CallExpression").count(), 2, "{tree}");
}
