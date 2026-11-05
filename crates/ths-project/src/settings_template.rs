//! What `ths init` writes: every setting, commented out, so the file
//! changes nothing yet and shows what can be set.

pub const SETTINGS_TEMPLATE: &str = r##"# Typed HaasScript project settings. Editors check the .ths and .hs files
# in this folder and below it. Every setting is optional: remove the "#"
# in front of one to change it from its default.

# Files to check and strip, as patterns relative to this file.
# include = ["**/*"]
# Patterns left out of include.
# exclude = []
# Where `ths strip` writes .hs files; beside each .ths file when not set.
# output = "build"
# The folder whose layout output copies.
# root = "src"

[check]
# Report values whose type is implicitly `any`.
# strict = true
# Warn where a name falls back to an outer one because a `local` of the
# same name ended with its block.
# shadowing = true
# Make `ths strip` write nothing when any file has an error.
# refuse_output_on_errors = true
"##;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Settings;

    #[test]
    fn the_template_changes_nothing() {
        assert_eq!(Settings::parse(SETTINGS_TEMPLATE), Ok(Settings::default()));
    }

    #[test]
    fn every_commented_setting_is_valid_when_uncommented() {
        let uncommented: String = SETTINGS_TEMPLATE
            .lines()
            .map(|line| match line.strip_prefix("# ") {
                Some(setting) if setting.contains(" = ") => setting,
                _ => line,
            })
            .collect::<Vec<_>>()
            .join("\n");
        let settings = Settings::parse(&uncommented).unwrap();
        assert!(settings.check.strict && settings.check.shadowing && settings.check.refuse_output_on_errors);
    }
}
