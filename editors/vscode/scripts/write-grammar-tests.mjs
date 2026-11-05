// Writes test/grammar/*.ths and *.hs from the cases below, putting each
// scope assertion under the text it names, so no caret is counted by hand.
// Run with `node scripts/write-grammar-tests.mjs` after changing a case.

import { writeFileSync } from "node:fs";
import { fileURLToPath } from "node:url";

const TYPED = "typed-haasscript";
const PLAIN = "haasscript";

/**
 * Each line is source, followed by its assertions: [text, scope], where
 * text is found in the line (the nth time for [text, scope, n]) and a scope
 * starting with "- " must be absent.
 */
const TYPED_LINES = [
  ['import { average, type Quote as Q } from "./math/averages"', [
    ["import", `keyword.control.import.${TYPED}`],
    ["average", `variable.other.readwrite.alias.${TYPED}`],
    ["type", `storage.type.type.${TYPED}`],
    ["as", `keyword.control.as.${TYPED}`],
    ["from", `keyword.control.from.${TYPED}`],
    ['"./math/averages"', `string.quoted.double.${PLAIN}`],
  ]],
  ['import * as series from "./series"', [
    ["*", `constant.language.import-export-all.${TYPED}`],
  ]],
  ["export local function latest(count: number, label?: string): Q", [
    ["export", `keyword.control.export.${TYPED}`],
    ["local", `storage.modifier.local.${PLAIN}`],
    ["latest", `entity.name.function.${PLAIN}`],
    [":", `keyword.operator.type.annotation.${TYPED}`],
    ["number", `support.type.primitive.${TYPED}`],
    ["?", `keyword.operator.optional.${TYPED}`],
    ["string", `support.type.primitive.${TYPED}`],
    ["Q", `entity.name.type.${TYPED}`],
  ]],
  ["    return prices:Get(count)", [
    ["prices", `- entity.name.type.${TYPED}`],
    ["Get", `entity.name.function.call.${PLAIN}`],
    ["Get", `- entity.name.type.${TYPED}`],
  ]],
  ["end", []],
  ['local quote = Load("quote") as Q | nil', [
    ["as", `keyword.operator.cast.${TYPED}`],
    ["Q", `entity.name.type.${TYPED}`],
    ["|", `keyword.operator.type.${TYPED}`],
    ["nil", `support.type.primitive.${TYPED}`],
  ]],
  ["type Pair<T> = { first: T, second?: (value: T) => string }", [
    ["type", `storage.type.type.${TYPED}`],
    ["Pair", `entity.name.type.alias.${TYPED}`],
    ["first", `variable.other.property.${TYPED}`],
    ["T", `entity.name.type.${TYPED}`, 2],
    ["value", `variable.parameter.${TYPED}`],
    ["=>", `storage.type.function.arrow.${TYPED}`],
  ]],
  ["interface Prices extends Base {", [
    ["interface", `storage.type.interface.${TYPED}`],
    ["Prices", `entity.name.type.interface.${TYPED}`],
    ["extends", `storage.modifier.extends.${TYPED}`],
    ["Base", `entity.name.type.${TYPED}`],
  ]],
  ["    close: number[]", [
    ["close", `variable.other.property.${TYPED}`],
    ["number", `support.type.primitive.${TYPED}`],
  ]],
  ["}", []],
  ["local function first<T>(values: T[]): T", [
    ["<", `punctuation.definition.typeparameters.begin.${TYPED}`],
    ["T", `entity.name.type.${TYPED}`],
  ]],
  ["    return values[1]", [
    ["values", `- entity.name.type.${TYPED}`],
  ]],
  ["end", []],
  ["if a < b and c > (d) then Log(a) end", [
    ["<", `keyword.operator.comparison.${PLAIN}`],
    [">", `keyword.operator.comparison.${PLAIN}`],
    ["d", `- entity.name.type.${TYPED}`],
    ["Log", `entity.name.function.call.${PLAIN}`],
  ]],
  ['local text: string = "a" .. "b" -- a comment', [
    ["string", `support.type.primitive.${TYPED}`],
    ["=", `keyword.operator.assignment.${PLAIN}`],
    ["..", `keyword.operator.concatenation.${PLAIN}`],
    ["-- a comment", `comment.line.double-dash.${PLAIN}`],
  ]],
  ["declare function CC_Example(value: number): number", [
    ["declare", `storage.modifier.declare.${TYPED}`],
    ["CC_Example", `entity.name.function.${PLAIN}`],
  ]],
];

const PLAIN_LINES = [
  ["local prices = ClosePrices()", [
    ["local", `storage.modifier.local.${PLAIN}`],
    ["ClosePrices", `entity.name.function.call.${PLAIN}`],
  ]],
  ["local average = SMA(prices, 10)", [
    ["10", `constant.numeric.decimal.${PLAIN}`],
  ]],
  ["--[[ a block", [
    ["a block", `comment.block.${PLAIN}`],
  ]],
  ["     comment ]]", [
    ["comment", `comment.block.${PLAIN}`],
  ]],
  ["local text = [==[a long", [
    ["[==[a long", `string.quoted.other.multiline.${PLAIN}`],
  ]],
  ["string]==]", [
    ["string]==]", `string.quoted.other.multiline.${PLAIN}`],
  ]],
  ["if average[1] ~= nil and not done then", [
    ["if", `keyword.control.${PLAIN}`],
    ["~=", `keyword.operator.comparison.${PLAIN}`],
    ["nil", `constant.language.${PLAIN}`],
    ["not", `keyword.operator.logical.${PLAIN}`],
  ]],
  ['    Log("value: " .. average[1])', [
    ['"value: "', `string.quoted.double.${PLAIN}`],
    ["value", `- entity.name.type.${TYPED}`],
  ]],
  ["end", []],
];

const COMMENT = "--";

/** The column of the nth whole-token match of `text` in `line`. */
function columnOf(line, text, occurrence) {
  const isWord = /^\w+$/.test(text);
  let from = 0;
  let seen = 0;
  for (;;) {
    const column = line.indexOf(text, from);
    if (column < 0) {
      throw new Error(`'${text}' (number ${occurrence}) is not in: ${line}`);
    }
    from = column + 1;
    const before = line[column - 1] ?? " ";
    const after = line[column + text.length] ?? " ";
    if (isWord && (/\w/.test(before) || /\w/.test(after))) {
      continue;
    }
    seen += 1;
    if (seen === occurrence) {
      return column;
    }
  }
}

/** A line of carets under `text`; a column under the comment uses `<-`. */
function assertion(line, [text, scope, occurrence = 1]) {
  const column = columnOf(line, text, occurrence);
  if (column > 0 && column < COMMENT.length) {
    throw new Error(`'${text}' starts under the comment marker but not at column 0: ${line}`);
  }
  if (column === 0) {
    return `${COMMENT} <${"-".repeat(text.length)} ${scope}`;
  }
  const padding = " ".repeat(column - COMMENT.length);
  return `${COMMENT}${padding}${"^".repeat(text.length)} ${scope}`;
}

function testFile(scopeName, description, lines) {
  const output = [`${COMMENT} SYNTAX TEST "${scopeName}" "${description}"`];
  for (const [line, assertions] of lines) {
    output.push(line, ...assertions.map((each) => assertion(line, each)));
  }
  return `${output.join("\n")}\n`;
}

const directory = new URL("../test/grammar/", import.meta.url);
writeFileSync(
  fileURLToPath(new URL("typed.ths", directory)),
  testFile(`source.${TYPED}`, "types are told apart from the plain script", TYPED_LINES),
);
writeFileSync(
  fileURLToPath(new URL("plain.hs", directory)),
  testFile(`source.${PLAIN}`, "a plain HaasScript file", PLAIN_LINES),
);
