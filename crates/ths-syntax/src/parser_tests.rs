use crate::test_support::outline;
use crate::{Dialect, parse};

/// Parses plain HaasScript, asserts it has no errors, and returns the
/// outline of the first statement.
fn first_statement(source: &str) -> String {
    let parsed = parse(source, Dialect::Plain);
    assert!(parsed.errors.is_empty(), "{:?}", parsed.errors);
    let root = parsed.syntax_node();
    let block = root.first_child().expect("block");
    outline(&block.first_child().expect("statement"))
}

fn error_messages(source: &str) -> Vec<String> {
    parse(source, Dialect::Plain).errors.into_iter().map(|error| error.message).collect()
}

#[test]
fn tree_text_is_the_source_exactly() {
    let source = "-- header\nlocal x = 1 -- one\n\nif x then\n  print('y')\nend\n";
    assert_eq!(parse(source, Dialect::Plain).syntax_node().to_string(), source);
}

#[test]
fn tree_text_is_the_source_exactly_even_with_errors() {
    let source = "local = = 3 end )";
    assert_eq!(parse(source, Dialect::Plain).syntax_node().to_string(), source);
}

#[test]
fn multiplication_binds_tighter_than_addition() {
    assert_eq!(
        first_statement("x = 1 + 2 * 3"),
        "\
AssignmentStatement
  NameExpression
    \"x\"
  \"=\"
  ExpressionList
    BinaryExpression
      LiteralExpression
        \"1\"
      \"+\"
      BinaryExpression
        LiteralExpression
          \"2\"
        \"*\"
        LiteralExpression
          \"3\"
"
    );
}

#[test]
fn concatenation_is_right_associative() {
    let tree = first_statement("x = a .. b .. c");
    let right_operand_is_nested =
        tree.contains("      \"..\"\n      BinaryExpression\n        NameExpression\n          \"b\"");
    assert!(right_operand_is_nested, "{tree}");
}

#[test]
fn power_binds_tighter_than_unary_minus() {
    let tree = first_statement("x = -a ^ 2");
    assert!(tree.contains("    UnaryExpression\n      \"-\"\n      BinaryExpression"), "{tree}");
}

#[test]
fn bang_equals_is_a_comparison() {
    let tree = first_statement("x = a != b");
    assert!(tree.contains("BinaryExpression"), "{tree}");
    assert!(tree.contains("\"!=\""), "{tree}");
}

#[test]
fn call_statement_wraps_the_call() {
    assert_eq!(
        first_statement("Log('hi')"),
        "\
CallStatement
  CallExpression
    NameExpression
      \"Log\"
    ArgumentList
      \"(\"
      ExpressionList
        LiteralExpression
          \"'hi'\"
      \")\"
"
    );
}

#[test]
fn method_call_statement() {
    let tree = first_statement("value:upper()");
    assert!(tree.starts_with("CallStatement\n  MethodCallExpression"), "{tree}");
}

#[test]
fn call_with_a_table_argument_needs_no_parentheses() {
    let tree = first_statement("Balance{coin = 'USDT'}");
    assert!(tree.contains("ArgumentList\n      TableConstructor"), "{tree}");
}

#[test]
fn local_function_statement() {
    let tree = first_statement("local function f(a, ...) return a end");
    assert!(tree.starts_with("LocalFunctionStatement"), "{tree}");
    assert!(tree.contains("Parameter\n        \"...\""), "{tree}");
}

#[test]
fn function_statement_with_a_method_name() {
    let tree = first_statement("function a.b:c() end");
    assert!(tree.contains("FunctionName\n    \"a\"\n    \".\"\n    \"b\"\n    \":\"\n    \"c\""), "{tree}");
}

#[test]
fn numeric_and_generic_for_are_told_apart() {
    assert!(first_statement("for i = 1, 10, 2 do end").starts_with("NumericForStatement"));
    assert!(first_statement("for k, v in pairs(t) do end").starts_with("GenericForStatement"));
}

#[test]
fn if_with_elseif_and_else_clauses() {
    let tree = first_statement("if a then elseif b then else end");
    assert!(tree.contains("ElseifClause"), "{tree}");
    assert!(tree.contains("ElseClause"), "{tree}");
}

#[test]
fn goto_and_label() {
    let parsed = parse("::top:: goto top", Dialect::Plain);
    assert!(parsed.errors.is_empty(), "{:?}", parsed.errors);
    let tree = outline(&parsed.syntax_node());
    assert!(tree.contains("LabelStatement"), "{tree}");
    assert!(tree.contains("GotoStatement"), "{tree}");
}

#[test]
fn lambda_with_parameters() {
    let tree = first_statement("f = |x, y| x * y");
    assert!(tree.contains("LambdaExpression\n      ParameterList\n        \"|\""), "{tree}");
}

#[test]
fn lambda_without_parameters() {
    let tree = first_statement("f = || 1");
    assert!(tree.contains("LambdaExpression"), "{tree}");
}

#[test]
fn table_fields_of_every_kind_with_mixed_separators() {
    let tree = first_statement("t = {1; a = 2, [3] = 4;}");
    assert!(tree.contains("PositionalField"), "{tree}");
    assert!(tree.contains("NamedField"), "{tree}");
    assert!(tree.contains("IndexedField"), "{tree}");
}

#[test]
fn multiple_assignment() {
    let tree = first_statement("a, b.c, d[1] = 1, 2, 3");
    assert!(tree.starts_with("AssignmentStatement"), "{tree}");
}

#[test]
fn parenthesis_on_the_next_line_continues_the_expression() {
    // As on the TradeServer: `y` and `(z)` join into the call `y(z)`, so
    // the `=` after `.w` is an error.
    let source = "local x = y\n(z).w = 1";
    let first_error = &parse(source, Dialect::Plain).errors[0];
    assert_eq!(&source[first_error.range], "=");
}

#[test]
fn semicolon_keeps_the_next_parenthesis_a_new_statement() {
    assert!(error_messages("local x = y;\n(z).w = 1").is_empty());
}

#[test]
fn lone_semicolons_are_empty_statements() {
    assert!(error_messages(";\nlocal a = 1;;\n;").is_empty());
}

#[test]
fn floor_division_is_a_syntax_error() {
    assert!(!error_messages("x = 7 // 2").is_empty());
}

#[test]
fn bare_expression_is_not_a_statement() {
    assert_eq!(error_messages("x"), ["expected '=' or a function call"]);
}

#[test]
fn assigning_to_a_call_is_an_error() {
    assert_eq!(error_messages("f() = 1"), ["cannot assign to this expression"]);
}

#[test]
fn missing_end_is_reported() {
    assert_eq!(error_messages("if a then"), ["expected 'end'"]);
}

#[test]
fn stray_end_is_reported_and_parsing_continues() {
    let parsed = parse("end x = 1", Dialect::Plain);
    assert_eq!(parsed.errors.len(), 1);
    assert!(outline(&parsed.syntax_node()).contains("AssignmentStatement"));
}
