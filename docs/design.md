# Design

Typed HaasScript adds TypeScript-style type annotations to HaasScript, the Lua-based scripting language of HaasOnline TradeServer, and checks scripts for type mismatches before they are uploaded.

## Decisions

| Area | Decision |
|---|---|
| Notation | TypeScript-style annotations written inline in the code: `name: type` on variables and parameters, `): type` for return types, `type` aliases, `interface` declarations, `T[]` arrays, `\|` unions, `?` optional, `<T>` generics and `as` casts. Annotations appear only in type positions. |
| Stripping | Like Node's built-in type stripping and TypeScript's `erasableSyntaxOnly`: every annotation is replaced with spaces, keeping newlines, so each line and column of the stripped script matches the typed source. A TradeServer error on line 42 points to line 42 of the source, with no source maps. Only syntax that can be erased is allowed; nothing generates code. |
| File extensions | `.ths` for typed source, `.d.ths` for declaration files. `ths strip` writes `.hs` files, which are uploaded to the TradeServer. |
| Project settings | `ths.toml`, TOML in place of `tsconfig.json`, for its comments and because it is the usual format of Rust tools. It is found as `tsc` finds `tsconfig.json`: in the current directory or the nearest one above, or named with `--project`. Its keys follow `tsconfig.json` where one exists: `include` and `exclude` (glob patterns; a directory means everything below it; default everything), `output` (`outDir`), `root` (`rootDir`), and a `[check]` table with `strict`, `shadowing` and `refuse_output_on_errors` (`noEmitOnError`). An unknown key is an error. Without file arguments a command works on the included files; with them, the settings still apply. As in TypeScript, `ths strip` writes even when there are errors unless `refuse_output_on_errors` is set, and replaces what is at the output path. The `ths-project` crate holds this, so the language server can share it. |
| Analyzer | One Rust core containing the parser, checker and stripper, built into a single `ths` binary with three commands: `ths check`, `ths strip` and `ths server` (a language server). |
| Editor support | A VSCode extension (a thin TypeScript client) and a JetBrains plugin (a thin Kotlin plugin), both driving `ths server` over the Language Server Protocol. |
| Built-in commands | `haasscript.d.ths` is generated from the TradeServer's own command list, then committed and reviewed by hand, like TypeScript's `lib.d.ts`. The generator and the server's raw data stay out of the repository. Hand corrections live in `overrides.d.ths`, whose declarations replace generated ones of the same name, so regenerating keeps them. Both files are built into `ths`, and every checked script sees them; problems inside them are never reported against the script. Each override rests on runtime behavior verified on the TradeServer (for example, `Random()` is listed as a number but returns a series). Custom commands are generated only when Haas ships them to every user; any other custom command can be a user's private script, so it is never generated. |
| Engine values | Enumeration constants, colors and series are engine objects, declared as opaque types (`declare type`). Each enumeration category has its own type extending `Enum`, so comparing constants of two categories (which stops the script, even inside `pcall`) is reported. A container's enumeration fields have their category's type too, such as `EnumPosition` for a position's `direction`. |
| Nil | Checked always, not only under `strict` (TypeScript checks it only under `strictNullChecks`). A value that may be nil must be typed `T \| nil` and narrowed (`if x then`, `x ~= nil`, `if not x then return end`) before use. `any` does not include nil; `any \| nil` must be narrowed too. Chosen because silent nils are HaasScript's most common hazard. A possible nil is reported as a warning, so it never fails `ths check`: it fails a script only on the run where the value is nil. |
| Strictness | TypeScript's default: types are inferred where possible, anything that cannot be inferred is `any` and unchecked, and an opt-in `strict` setting (`strict` in `ths.toml`, or `ths check --strict`) reports implicit `any`, as TypeScript's `noImplicitAny`: a parameter with neither an annotation nor a type from its context (a callback's parameters take the expected function's), and `local x` with neither an annotation nor a value. It applies to `.ths` files only, since a plain script cannot hold annotations to fix it. Not reported yet: an unannotated `...`, which contextual typing does not reach, and `for ... in` variables, whose iterator types are not inferred. Plain `.lua` and `.hs` scripts are out of scope for type corrections: they are read for the types a typed script can use, and they report only what stops that: syntax errors, reads of names nothing defines, and globals a custom command creates, which the server refuses to compile. To see every error and warning in one, rename it to `.ths`. |

## Example

Typed source, `trend-bot.ths`:

```lua
interface Signal {
  direction: EnumSignal
  strength: number
}

local function average(prices: number[], period: number): number
  ...
end

local current: Signal = {direction = SignalLong, strength = 1}
```

After `ths strip`, `trend-bot.hs` has the same lines and columns, with the annotations blanked out:

```lua




local function average(prices          , period        )        
  ...
end

local current         = {direction = SignalLong, strength = 1}
```

## Typed syntax

What the parser accepts in `.ths` and `.d.ths` files, on top of HaasScript:

| Form | Example |
|---|---|
| Variable annotation | `local x: number = 1`, `for i: number = 1, 3 do`, `for key: string, value: number in pairs(t) do` |
| Parameters | `function f(a: number, b?: string, ...: boolean): number` |
| Generics | `function identity<T extends number = number>(value: T): T` |
| Explicit type arguments | `identity<<number>>(42)`, `t:get<<number>>('key')`, `local f = identity<<number>>`. Luau's syntax: `f<number>(x)` already means `(f < number) > (x)` in Lua, while `<<` cannot continue any Lua 5.2 expression. |
| Several return values | `function bounds(): (number, number)`, `() => (number, string)`, and any number more with Luau's `...`: `(number, ...string)`, or `...string` alone. A list needs two or more types, or a final `...T`, and is valid only as a return type; `(number)` is one parenthesized type. |
| Lambda parameters | `\|x: number\| x * 2`. A union must be parenthesized, `\|x: (A \| B)\| x`, because `\|` closes the parameter list. |
| Cast | `a + b as number`, which binds like a comparison, as in TypeScript: `(a + b) as number`. A call or `...` may be cast to several values, as in Teal: `local a, b = table.unpack(t) as (number, string)`. |
| Type alias | `type Side = "long" \| "short"` (a leading `\|` is allowed) |
| Interface | `interface Bot<T> extends Base { name: string; size?: number; 1: string; step(a: T): nil; [key: string]: any }` (members separated by `,`, `;` or nothing; a numeric name such as `1` types that index, so the built-in containers type `position[2]` as their second field) |
| Declarations | `declare function ClosePrices(interval?: number): number[]` and `declare SignalLong: EnumSignal`, for declaration files |
| Opaque types | `declare type EnumSignal extends Enum`: a type known only by its name, which fits itself and its bases and has no members. A body may hold one call signature, as in a TypeScript interface: `declare type EnumColor extends Enum { (opacity: number): EnumColor }` makes `Red(50)` a call while `Red` stays an `EnumColor`. A base's call signature is inherited. |
| Types | names with arguments `Map<K, V>`, arrays `T[]`, unions `A \| B`, intersections `A & B`, literals `"long"`, `1`, `true`, `nil`, functions `<T>(value: T) => T`, object types `{ a: T }`, parentheses |

`type`, `interface`, `declare`, `extends` and `as` are contextual: they are only keywords in the positions above, so `type(x)` stays a function call. In plain HaasScript files, annotations are reported as "only allowed in .ths files", and `type`, `interface`, `declare` and `as` are ordinary names.

## Code without annotations

Annotated code follows TypeScript's rules. Where there is no annotation, Lua idioms that TypeScript's rules would reject are accepted, much as TypeScript is more lenient when it checks JavaScript files. Found by running the checker over about 800 real HaasScript files, which cut its diagnostics from about 2,400 to about 75, nearly all of them genuine possible nils.

- **Tables are built field by field.** A table type inferred from a constructor is open: reading or assigning a field it did not start with is `any`, not an error (`local order = {}` then `order.price = 1`). A field initialized to `nil` is `any`. An inferred table is never the expected type for a later table constructor.
- **Inferred variables and fields accept nil.** `local current = 1` then `current = nil` is fine; afterwards `current` is narrowed to nil, so `current + 1` reports "possibly nil".
- **An inferred type stays in a `.ths` file.** A variable or field inferred as one type and later assigned a value of another, such as a number padded into text (`month = '0'..month`), is an error, as in TypeScript: reusing a variable for a different kind of value is exactly what the checker is for, and an annotation (`local month: number | string`) states the intent and clears it. In a plain file, which cannot hold that annotation, the variable becomes `any` instead. The one exception in both is a math helper's result in a number variable (see Series), since the server treats that series as a number.
- **An unchecked value replaces an inferred type.** `local result = 0` then `result = lookup.anything()` makes `result` `any` from there on, since its type came only from its first value. An annotated variable keeps its type.
- **A plain file leaves what it cannot tell as `any`.** A `.lua` or `.hs` file is out of scope for type corrections: its types are inferred so that a typed script can use it without changing it, such as a bot written in `.ths` that calls a custom command written in `.hs`, perhaps from a library its author does not maintain. It cannot hold annotations, so where the checker cannot tell a type, it does not guess one. `flag and value` or `flag and a or b`, Lua's stand-in for a ternary operator, is `any` there when either operand is `any`, as in TypeScript, so `local maxCheck = useMax and (base * leverage)` with `useMax` from `Input` passes to `Round`. A `.ths` file keeps the exact result, `nil | false | number`, since its author can annotate.
- **Unannotated parameters are optional.** Lua passes nil for a missing argument, so `f(1)` may call `function f(a, b)`. Inside the function such a parameter is an inferred `any`.
- **Undeclared names are `any` when the script makes them known.** The TradeServer refuses to compile a script that reads a name nothing defines, wherever the read is, verified on the TradeServer, so with the built-in declarations such a read is an error. A name is known when a local of it is in scope, the declarations declare it, the script assigns it as a global or defines it with `function name()` anywhere (even after the read), or a string literal is exactly the name. A comment, a table key, a field, a longer string, a parameter or a `local` in another block does not make it known. Names starting with `CC_` are known, since the server knows every custom command its owner has saved. Without the built-in declarations the checker cannot know the server's names, so it does not report unknown ones.
- **A `local` ends with its block**, as in Lua 5.2, verified on the TradeServer. A read after the block falls back to an outer or global definition of the name. That is often intended, so it is reported only with the opt-in `shadowing` setting (`ths check --shadowing`), as a warning.

Narrowing applies to variables and to field paths from them (`if order.price ~= nil then ... order.price`). Assigning to the variable, or assigning anywhere in a loop or branch, drops those narrowings.

## Generics

As in TypeScript, and where TypeScript's syntax does not fit Lua, as in Luau:

- **Declarations.** `type` aliases, interfaces and functions take type parameters with optional constraints and defaults: `interface Box<T = number> { value: T }`, `function largest<T extends number>(values: T[]): T`. A reference must give every type argument without a default (`Box<string>`, or `Box` alone here).
- **Inside the function** a type parameter is opaque: only `T` itself (or `any`) fits a `T`, and `T` is usable as its constraint, so `value * 2` is fine when `T extends number`.
- **Calls infer the type arguments** from the arguments, unless they are given explicitly with `f<<number>>(x)`. Function arguments are checked last, typed by what the others inferred, so in `map(prices, |price| price * 2)` the lambda's `price` is a `number` and `map` returns `number[]`. Several candidates for one type parameter form a union. A type parameter with no candidates takes its default, else `unknown`. An inferred type argument must satisfy its constraint: "Type 'string' does not satisfy the constraint 'number'."
- **Explicit type arguments bind the leading type parameters,** as in Luau: for `second<A, B>(a: A, b: B)`, `second<<number>>(1, 'a')` binds `A` and still infers `B`. Each must satisfy its constraint, and more than the function declares is an error.
- **A generic function passed where a function type is expected** has its type parameters replaced by their constraints (or `any`); TypeScript would infer them from the expected type.

## Multiple return values

A return type may list several values, `): (number, string)`, and the checker follows Lua's rules for them (each rule verified on the TradeServer with `select('#', ...)`, including that parentheses truncate and that a nil in the middle counts as a value):

- **Each `return` is checked by position.** A missing value is nil, so `return 1` in a function returning `(number, string)` reports that nil is not a `string`. More values than declared is an error. Without an annotation, the return type is inferred position by position across every `return`.
- **A call gives one value inside an expression,** its first: `pair() + 1`, or `(pair())`.
- **A call or `...` at the end of a list spreads** over the remaining targets: `local a, b = pair()`, `a, b = pair()`, `return pair()` and `g(1, pair())`. A position past the last value is nil. A call to something unchecked, and `...`, supply any number of values, so they are never too few.
- **Rest values, `...string`, may be absent.** A target of its own receives `string | nil`; passed on into a `...: string` parameter or a `...string` return, a rest value stays a `string`, since it is absent rather than nil. A call with rest values supplies an unknown number of values, so it is never too few.
- **A list cast types every value of a call,** `f() as (number, ...string)`, and spreads like the call itself; inside an expression it gives its first value. Only a call or `...` can be cast to a list.
- **Too few values for the targets is an error, as in Luau.** `local a, b, c = pair()`, where `pair` returns exactly two values, reports "The call returns 2 value(s), but 3 are required here.": `c` would always be nil. Only a known, fixed count is checked, so calls with rest values, unchecked commands and `...` are exempt. (It added no diagnostics to the corpus of real scripts.)
- **Extra values are dropped, as in Lua.** A function returning `(number, string)` fits where `() => number` is expected, and `g(pair())` with a one-parameter `g` is fine. Writing more argument expressions than parameters is still an error.

## Series

`HaasNumberCollection`, the engine's series (price data, indicator results, `HNC()`), has operator rules of its own. Each rule was verified on the TradeServer:

- **Indexing gives a series.** `close[3]` is the series from element 3 onward, not a number; `ArrayGet(close, 3)` gives the number.
- **A series is read-only.** `close[1] = 5` is an error, as it is on the server; `ArrayUnshift` and `ArrayAdd` give a new series.
- **Arithmetic is element-wise.** `+ - * / %` and unary minus with a series on either side give a series. The other operand may be a number, a string (converted at runtime) or a series; a boolean is an error, because it stops the script. `^` on a series is an error.
- **Comparisons use the newest element.** `<`, `>`, `<=` and `>=` accept a number or a series on each side. Anything else is an error, because the server silently gives `false`. A series may equal a number; comparing it with a string, boolean, enumeration constant or table is reported as having no overlap.
- **`#` gives the length.** On a series it is a number; on an enumeration constant or a color it is an error, as it is on the server.
- **Concatenation needs a string on the other side.** `'close ' .. close` gives the newest element as text. With a number or another series on the other side the server silently drops one side (`close .. 1` gives `1`), so that is an error. `..` groups from the right, so `'x ' .. close .. 1` is reported for its inner `close .. 1`. A value that may be a number or a series is checked for each: beside a string it is fine.
- **A number variable may take a series.** The math helpers (`SubPerc`, `Min`, `Pow`, …) return a series of one element even for plain numbers, and scripts often start a variable at `0` and assign such a result to it later. That series works like a number in arithmetic, comparisons, command arguments and concatenation with a string, so the assignment is not an error: it widens a variable inferred as a number to `number | HaasNumberCollection` from that point, and a variable inferred as a series takes a number the same way. Only the assignment that brings in the other kind widens the variable, so a counter that only ever holds numbers stays a number. The series is reported where it breaks: as a `for` bound and as an argument to Lua's `math` and `string` functions (errors on the server), and as a table key, where the server silently treats it as a key of its own, so the entry is never found. A field of a table whose type was inferred from its constructor is widened the same way (`state.change = SubPerc(…)` after `local state = {change = 0}`), and later reads of the field see the widened type. An annotated variable, and a field of a table with an annotated or declared type, keeps its type.

## Named-argument tables

A command call may end with a table of `name = value` fields, such as `PlaceGoLongOrder(price, amount, {type = MarketOrderType, note = 'entry'})`. The table may be written in place, held in a variable, or returned by a function call that gives only the table. A table with a positional value is an ordinary value, not named arguments. Each rule was verified on the TradeServer:

- **The table fills parameters by name.** It sits in an optional parameter's slot and fills that parameter or any later one; each value is checked against its parameter's type. A table for a parameter that takes tables itself, such as `Plot`'s options, is an ordinary argument.
- **A key that matches nothing is reported.** That covers a misspelled key and a key naming an earlier parameter. Beside a key that matches, the server ignores it and the call still works, so it is a warning: scripts that reuse one options table across several commands carry such keys. When no key matches, the server reads the table as the slot's value, so it is an error: a misspelled key's value silently becomes a tooltip, or the script stops.
- **A table in a required parameter's slot is an error.** The server reads it as that parameter and stops the script.

## Text conversion

A TradeServer command converts any value passed for a string parameter to text, verified on the TradeServer: a number becomes its text (a small one in exponent notation, so 0.00001 becomes `1E-05`), an enumeration constant its name, a color its `rgba(...)` text and a boolean its text. The built-in declarations still type those parameters as `string`, and passing anything else to a command is a warning rather than an error. A Lua function receives the value itself, so passing a number to its `string` parameter stays an error.

## Containers as numbers

A TradeServer command given a result container for a number parameter uses the container's first field, verified on the TradeServer: `CurrentPrice()` gives its open price, `MACD(...)` its newest `macd` value, `BBANDS(...)` its newest upper band. A container whose first field is not a number, such as `Compare(...)`, stops the script. So passing a container to a command's number parameter is a warning rather than an error when its first field is a number or a series, and stays an error otherwise. For `CurrentPrice()` the warning names `.close`, `.bid` and `.ask`, since the open price is rarely what a script means. A container is an interface from the built-in declarations; an interface a script declares is an ordinary table, so passing it stays an error. Outside a command argument (a `number` variable, or a Lua function's parameter), a container is the whole container, so assigning it stays an error.

## Constants compared with text

An enumeration constant compared with a string compares the constant's name, verified on the TradeServer: `GetType(1) == 'NumberDataType'` and `SignalLong == 'SignalLong'` are true, in either order. The comparison is case-sensitive, any other text is simply unequal, and colors never equal a string. So comparing a constant with a string literal that names a constant of the same category is allowed. Comparing it with any other literal is reported as having no overlap, since the result never changes: `GetType(name) == 'string'` is always false, because `GetType` gives `TextDataType`, not Lua's type name. A string that is not a literal may hold a name, so comparing a constant with it is allowed.

## Indexing a value that is not a table

Indexing a function, a number or a boolean with brackets stops the script, verified on the TradeServer, so it is an error, reading or writing. A command's name is a plain function, so `CurrentPrice[1]` for `CurrentPrice()[1]` is the usual slip, and its message says to call the command first. Indexing a string with brackets gives nil on the server and is not reported.

## Custom commands

A custom command is defined in one saved script and called from others as `CC_<Name>`, with no import: the server makes every saved command of its owner available. Its interface is written as calls in its own code, so the checker reads it from there:

- **Every file in the project that calls `DefineCommand` declares `CC_<Name>` for all the others**, as TypeScript's global scripts see one another. The name is the `DefineCommand` name, not the file's. Plain files take part: their signatures are inferred, so a `.ths` bot can call a command written in `.hs` without changing it.
- **Commands not in the project come from `.d.ths` files**, as TypeScript's `@types` packages. The server reports a command's parameters, their types and whether each is required when it compiles the command, so a user can declare their own commands in a local, ignored file; those are never committed.
- **Built:** `ths check` and `ths strip` read every included file (and every file named) for a `DefineCommand` and declare its command for all the others, and load the project's `.d.ths` files as declarations. Only literal arguments are read: a parameter whose type or `isRequired` is not a literal is `any` and optional, `DefineOutputIndex` entries written with variables give no container, and a command whose name is not a literal is not declared, so calls to it stay `any`. A command defined in two files is reported, and the first file's is used. A call returns the type the `DefineOutput` constant allows, narrowed to the type of the value the command gives (checked against the built-in and project declaration files) when that value is one the constant passes through: `DefineOutput(ListNumberType, SMA(…))` returns a `HaasNumberCollection`, and a `DefineOutputIndex` entry holds the type of the table field at its position. A value the server would convert (a number for `StringType`), an unchecked `any`, or a type only the command declares leaves the constant's type.
- **Inside the command, a `DefineParameter` call has the type the parameter receives** (verified): `NumberType` a number, `StringType` a string, `BooleanType` a boolean, `ListNumberType` a series or the default (a left-out parameter, and every parameter on the compile pass, holds its default unchanged, so a `{1, 2}` default gives `HaasNumberCollection | number[]` and a `ClosePrices()` default a `HaasNumberCollection`), and `DynamicType` and `ListDynamicType` `any`. An `EnumType` parameter has its default's category (`EnumMovingAverages` for `SmaType`), and callers must pass that category: the server passes a constant of any category, but comparing it with one of another category stops the script. Only a call that reads the built-in `DefineParameter` with a type constant counts.
- **A command may not create globals** (verified): the server refuses to compile a command file that assigns a name that is neither a local nor a built-in, or has a `function name()` statement for one, anywhere in the file, even in a function never called or a branch never taken. `ths check` reports it at the first assignment of each name, as the server reports each name once, and reports it in plain files too, since the command cannot be saved. A field of a global table (`string.extra = 1`) and a built-in name (`Log = Log`) are allowed. A file is a command when it calls `DefineCommand`.
- **Commands saved only on the server** are declared by the user in a `.d.ths` file in their project, kept out of version control since the commands are private. What a caller passes follows the built-in commands' mapping; what it receives follows the verified custom command facts (a `ListNumberType` output is `number[] | HaasNumberCollection`, an `EnumType` output `Enum` unless its suggestions name categories, `DefineOutputIndex` entries a container). A command also defined in the project is declared again from its source, which is read later and wins. Each file sees every project declaration but its own, so a declaration file never clashes with itself.
- **The signature, from facts verified on the TradeServer:** parameters in `DefineParameter` order; one whose `isRequired` is false is optional. Inside the command a `NumberType` parameter is a number and a `StringType` parameter is text, since the server converts each argument; a caller may pass a series or numeric text for `NumberType` and a number for `StringType`; the declarations accept a number or a series for `NumberType`, and give a warning for a number passed as text, as for built-in commands. Calling with too few or too many arguments, nil for a required parameter, or a named-argument table is an error: it fails the backtest. The return type is the type of the value given to `DefineOutput`: `BooleanType` arrives as a boolean, `EnumType` as the constant, `ListNumberType` as the value unchanged, and a `DynamicType` output holding a table makes the command invalid. A table with `DefineOutputIndex` entries reaches the caller as a container read by those names or by index, positions following the table constructor's order.

## Stripping rules

- **Keep statements apart.** Lua has no statement separator, so a statement starting with `(` continues the expression before it (verified on the TradeServer: `local x = y` followed by `(z).w = 1` on the next line is read as `local x = y(z).w = 1`). In the typed source, blanked text can be what ends a statement: a `type`, `interface` or `declare` statement, or the `as Type` at the end of an expression. When the next statement starts with `(`, the stripper writes `;` into the first blanked character instead of a space. A lone `;` is an empty statement on the TradeServer (verified), and lines and columns still match the source.
- **A cast does not truncate.** The checker reads `pair() as number` as one value, but stripping leaves `pair()`, which still passes every value it returns. Where those values would reach something (the end of an argument list, a `return` or a table constructor, or a local or assignment with more targets than expressions), a cast on a call known to return several values, or on `...`, is an error that asks for `(pair()) as number`: parentheses truncate in Lua and survive stripping. Luau's `::` truncates at runtime, so Luau has no such rule.
- **ASCII only.** The TradeServer deletes every non-ASCII character when a script is saved (verified: `Log("é")` saves as `Log("")`), so `ths check` warns on any non-ASCII character in a typed or plain file. A string literal type containing one, such as `"€"`, can never match its runtime value.

## Imports

Built so far: the syntax; every rule one file settles (placement, a module's declarations-only top level, no `Define…` calls in a module, duplicate and conflicting names, export lists); resolving paths, with an import that leads nowhere, an imported file with no exports and circular imports reported at the import's path; and types across files: each import has its export's type, a module's types are shown as `import("lib/quotes").Quote` in the files importing it, `type` imports must name types and other imports values, imports and exported variables are read-only, and an unused import is an error. A module on an import cycle is imported as `any`. Bundling: `ths strip` writes nothing for a module, and writes a trade script that imports values with every module it needs, in dependency order; type-only imports add no module. Bundled files are stripped compactly, with removed ranges cut out instead of blanked, and each file is labelled with a comment giving its path from the script's directory. A script importing no values is stripped as before, its lines matching the source. Verified on the server: bundled bots and bundled custom commands compile and run.

The server has no `require`: a script is one uploaded file. Typed HaasScript adds TypeScript's `import` and `export`, and `ths strip` bundles a script with the modules it imports into one `.hs` file. No typed or bundled script exists on a server yet, so every rule starts strict; any can be loosened later without breaking anyone.

- **A file's exports decide its role.** A `.ths` file with an `export` is a module: it is never uploaded, and `ths strip` writes nothing for it. A file without one is a trade script, a bot or a custom command, and may import modules; `ths strip` writes it with every module it imports, directly or not, bundled in.
- **Custom commands are never imported.** The server already makes every saved command available by its `CC_` name. A custom command may import modules, which are bundled into it. Modules may not call `DefineCommand`, `DefineParameter` or `DefineOutput`: a command's interface stays in its own file, where the server and the checker read it.
- **Only files with `export` syntax can be imported**, so a plain `.lua` or `.hs` file never is (Lua has no `export`): to use one, rename it to `.ths` and mark its exports.
- **Exports are on top-level declarations**, `export local function average(...)`, `export local limit = 10`, `export type Prices = ...`, or name an existing local, `export { helper as assist }`.
- **Imports are named:** `import { average, sum as total } from "./math/averages"`, `import { type Prices } from ...` (removed when bundling), and `import * as averages from ...` (the exports as one table). No default exports, re-exports or side-effect imports.
- **Paths are relative to the importing file**, start with `./` or `../`, and leave out the extension; they name a `.ths` file. No bare names, no directory index files.
- **An import is a read-only copy** of the export, taken when the module finishes loading. Assigning to an import is an error, and so is reassigning an exported variable, since importers would not see it (TypeScript's live bindings would). Changing state is shared through a table's fields or through functions.
- **A module's top level holds only declarations and imports:** `local` and `local function` declarations, exported or not, `type` and `interface` declarations, and `import` statements. A statement that does something, such as a `Log` call, an `if` or a loop, is an error there, so importing a module has no hidden effect on every tick; a declaration's value may still call commands (`local close = ClosePrices()`).
- **Strict rules:** imports come first in the file, each name is imported once, an import cannot share a name with a local, an unused import is an error, and circular imports are an error.
- **The bundle** puts every module first, in dependency order, each wrapped in a function that runs once and returns its exports table, then the script, with each import rewritten as `local` bindings. No globals are used, so a command can be bundled too. Its lines do not match the typed source: server errors are read against the bundled `.hs`, which the user keeps. Each module runs on every run of the script, before the script's own code, as the whole script runs on every tick.
- **Verified on the server**: a generated bot bundle and a generated custom command bundle compiled and backtested, every tick giving the expected values. A command's `DefineParameter` and `DefineOutput` are still found after the module wrappers. A module's state starts over each tick, like the rest of the script.
