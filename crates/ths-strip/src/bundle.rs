//! Joins a stripped trade script and the stripped modules it imports into
//! one plain file (design.md, "Imports"):
//!
//! ```text
//! -- math/averages.ths
//! local module_averages = (function()
//! <the module, stripped>
//! return { average = average };
//! end)();
//! -- bots/trend.ths
//! local average, total = module_averages.average, module_averages.sum;
//! <the script, stripped>
//! ```
//!
//! Each module runs once, inside its own function, and its exports table
//! is a `local`: the bundle creates no global. A file's imports become one
//! `local` statement, so every value is read before any import's name is
//! bound. Each generated statement ends with `;`, so a following line that
//! starts with `(` is not read as a call. The bundle's lines do not match
//! the typed source: server errors are read against it.

const MODULE_TABLE_PREFIX: &str = "module_";

/// A local the bundle binds from a module's exports table.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Binding {
    pub local: String,
    /// The index of the module, in the order given to `bundle`.
    pub module: usize,
    /// The export it copies; `None` binds the whole table (`* as name`).
    pub exported: Option<String>,
}

/// A stripped file to bundle.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BundlePart {
    /// The file's path as shown to the owner, written above its code.
    pub label: String,
    pub code: String,
    /// The value imports of the file, in source order.
    pub bindings: Vec<Binding>,
}

/// A stripped module to bundle, and what its exports table holds.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BundleModule {
    pub part: BundlePart,
    /// Each exported value: its name in the table, and the local holding
    /// it.
    pub exports: Vec<(String, String)>,
}

/// The plain file holding `script` and `modules`, which come in
/// dependency order: a module binds only modules before it.
pub fn bundle(modules: &[BundleModule], script: &BundlePart) -> String {
    let tables = table_names(modules);
    let mut output = String::new();
    for (module, table) in modules.iter().zip(&tables) {
        push_label(&mut output, &module.part.label);
        output.push_str(&format!("local {table} = (function()\n"));
        push_code(&mut output, &module.part, &tables);
        output.push_str(&format!("return {};\nend)();\n", exports_table(&module.exports)));
    }
    push_label(&mut output, &script.label);
    push_code(&mut output, script, &tables);
    output
}

fn push_label(output: &mut String, label: &str) {
    output.push_str(&format!("-- {label}\n"));
}

/// The file's import bindings, then its code.
fn push_code(output: &mut String, part: &BundlePart, tables: &[String]) {
    if !part.bindings.is_empty() {
        let names: Vec<&str> = part.bindings.iter().map(|binding| binding.local.as_str()).collect();
        let values: Vec<String> = part
            .bindings
            .iter()
            .map(|binding| match &binding.exported {
                Some(exported) => format!("{}.{exported}", tables[binding.module]),
                None => tables[binding.module].clone(),
            })
            .collect();
        output.push_str(&format!("local {} = {};\n", names.join(", "), values.join(", ")));
    }
    output.push_str(&part.code);
    if !part.code.ends_with('\n') {
        output.push('\n');
    }
}

fn exports_table(exports: &[(String, String)]) -> String {
    if exports.is_empty() {
        return "{}".to_string();
    }
    let fields: Vec<String> = exports.iter().map(|(exported, local)| format!("{exported} = {local}")).collect();
    format!("{{ {} }}", fields.join(", "))
}

/// `module_<file name>` for each module, numbered when two share a name.
fn table_names(modules: &[BundleModule]) -> Vec<String> {
    let mut names: Vec<String> = Vec::with_capacity(modules.len());
    for module in modules {
        let base = format!("{MODULE_TABLE_PREFIX}{}", identifier_from(&module.part.label));
        let mut name = base.clone();
        let mut number = 2;
        while names.contains(&name) {
            name = format!("{base}_{number}");
            number += 1;
        }
        names.push(name);
    }
    names
}

/// The label's file name without its extension, each character a Lua
/// name cannot hold replaced by `_`.
fn identifier_from(label: &str) -> String {
    let file_name = label.rsplit(['/', '\\']).next().unwrap_or(label);
    let stem = file_name.split('.').next().unwrap_or(file_name);
    stem.chars().map(|character| if character.is_ascii_alphanumeric() { character } else { '_' }).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn part(label: &str, code: &str, bindings: Vec<Binding>) -> BundlePart {
        BundlePart { label: label.to_string(), code: code.to_string(), bindings }
    }

    fn binding(local: &str, module: usize, exported: Option<&str>) -> Binding {
        Binding { local: local.to_string(), module, exported: exported.map(str::to_string) }
    }

    #[test]
    fn a_script_without_imports_is_its_code() {
        assert_eq!(bundle(&[], &part("bot.ths", "Log(1)\n", vec![])), "-- bot.ths\nLog(1)\n");
    }

    #[test]
    fn each_module_runs_in_its_own_function_before_the_script() {
        let averages = BundleModule {
            part: part("math/averages.ths", "local function average(a, b) return (a + b) / 2 end\n", vec![]),
            exports: vec![("average".to_string(), "average".to_string())],
        };
        let script = part(
            "bot.ths",
            "Log(mean(1, 3))\n",
            vec![binding("mean", 0, Some("average")), binding("averages", 0, None)],
        );
        assert_eq!(
            bundle(&[averages], &script),
            "-- math/averages.ths\n\
             local module_averages = (function()\n\
             local function average(a, b) return (a + b) / 2 end\n\
             return { average = average };\n\
             end)();\n\
             -- bot.ths\n\
             local mean, averages = module_averages.average, module_averages;\n\
             Log(mean(1, 3))\n"
        );
    }

    #[test]
    fn a_module_binds_the_modules_before_it() {
        let first = BundleModule {
            part: part("a/limits.ths", "local limit = 1", vec![]),
            exports: vec![("limit".into(), "limit".into())],
        };
        let second = BundleModule {
            part: part("b/limits.ths", "local double = limit * 2", vec![binding("limit", 0, Some("limit"))]),
            exports: vec![("double".into(), "double".into())],
        };
        let bundled =
            bundle(&[first, second], &part("bot.ths", "Log(double)", vec![binding("double", 1, Some("double"))]));
        assert!(
            bundled.contains(
                "local module_limits_2 = (function()\nlocal limit = module_limits.limit;\nlocal double = limit * 2\n"
            ),
            "{bundled}"
        );
        assert!(bundled.ends_with("local double = module_limits_2.double;\nLog(double)\n"), "{bundled}");
    }

    #[test]
    fn a_table_name_holds_only_name_characters() {
        assert_eq!(identifier_from("lib/price-tools.ths"), "price_tools");
    }
}
