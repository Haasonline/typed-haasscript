use std::path::{Path, PathBuf};

use tempfile::TempDir;

use crate::{Project, ProjectError, Settings, source_files_under};

/// A project directory holding `files` (empty), with `settings` as its
/// `ths.toml`.
fn project(settings: &str, files: &[&str]) -> (TempDir, Project) {
    let directory = tempfile::tempdir().unwrap();
    std::fs::write(directory.path().join("ths.toml"), settings).unwrap();
    for file in files {
        let path = directory.path().join(file);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, "").unwrap();
    }
    let project = Project::load(directory.path()).unwrap();
    (directory, project)
}

/// Each of `files` relative to `directory`, written with `/` on every
/// platform, so the expected paths read the same on Windows.
fn relative(directory: &Path, files: Vec<PathBuf>) -> Vec<String> {
    files
        .iter()
        .map(|file| {
            let parts: Vec<String> = file
                .strip_prefix(directory)
                .unwrap()
                .components()
                .map(|part| part.as_os_str().to_string_lossy().into_owned())
                .collect();
            parts.join("/")
        })
        .collect()
}

#[test]
fn empty_settings_take_the_defaults() {
    assert_eq!(Settings::parse("").unwrap(), Settings::default());
}

#[test]
fn check_settings_are_read() {
    let settings = Settings::parse("[check]\nstrict = true\nrefuse_output_on_errors = true").unwrap();
    assert!(settings.check.strict && settings.check.refuse_output_on_errors && !settings.check.shadowing);
}

#[test]
fn an_unknown_key_is_an_error() {
    assert!(Settings::parse("stric = true").unwrap_err().contains("unknown field"));
}

#[test]
fn everything_is_included_by_default() {
    let (directory, project) = project("", &["bot.ths", "lib/helper.lua", "notes.txt", "old.hs"]);
    assert_eq!(relative(directory.path(), project.included_files().unwrap()), ["bot.ths", "lib/helper.lua", "old.hs"]);
}

#[test]
fn a_directory_pattern_includes_everything_below_it() {
    let (directory, project) = project("include = [\"src\"]", &["src/bot.ths", "src/deep/helper.hs", "other.ths"]);
    assert_eq!(relative(directory.path(), project.included_files().unwrap()), ["src/bot.ths", "src/deep/helper.hs"]);
}

#[test]
fn a_single_star_stays_in_one_directory() {
    let (directory, project) = project("include = [\"src/*.ths\"]", &["src/bot.ths", "src/deep/helper.ths"]);
    assert_eq!(relative(directory.path(), project.included_files().unwrap()), ["src/bot.ths"]);
}

#[test]
fn exclude_leaves_files_out() {
    let (directory, project) = project("exclude = [\"vendor\"]", &["bot.ths", "vendor/library.hs"]);
    assert_eq!(relative(directory.path(), project.included_files().unwrap()), ["bot.ths"]);
}

#[test]
fn the_output_directory_is_left_out() {
    let (directory, project) = project("output = \"build\"", &["bot.ths", "build/bot.hs"]);
    assert_eq!(relative(directory.path(), project.included_files().unwrap()), ["bot.ths"]);
}

#[test]
fn a_stripped_file_beside_its_source_is_left_out() {
    let (directory, project) = project("", &["bot.ths", "bot.hs", "plain.hs"]);
    assert_eq!(relative(directory.path(), project.included_files().unwrap()), ["bot.ths", "plain.hs"]);
}

#[test]
fn hidden_directories_are_left_out() {
    let (directory, project) = project("", &["bot.ths", ".git/hook.lua"]);
    assert_eq!(relative(directory.path(), project.included_files().unwrap()), ["bot.ths"]);
}

#[test]
fn a_directory_argument_lists_its_source_files() {
    let (directory, _) = project("", &["src/bot.ths", "src/readme.md"]);
    let files = source_files_under(&directory.path().join("src"), &[]);
    assert_eq!(relative(directory.path(), files), ["src/bot.ths"]);
}

#[test]
fn find_looks_in_the_directories_above() {
    let (directory, project) = project("", &["src/deep/bot.ths"]);
    assert_eq!(Project::find(&directory.path().join("src/deep")).unwrap(), Some(project));
}

#[test]
fn a_bad_settings_file_names_its_path() {
    let directory = tempfile::tempdir().unwrap();
    std::fs::write(directory.path().join("ths.toml"), "include = 1").unwrap();
    assert!(matches!(Project::load(directory.path()), Err(ProjectError::Settings { .. })));
}

#[test]
fn stripped_files_go_beside_their_sources_by_default() {
    let (directory, project) = project("", &["src/bot.ths"]);
    let stripped = project.stripped_paths(&[directory.path().join("src/bot.ths")]);
    assert_eq!(relative(directory.path(), stripped), ["src/bot.hs"]);
}

#[test]
fn output_keeps_paths_below_the_common_directory() {
    let (directory, project) = project("output = \"build\"", &["src/bot.ths", "src/lib/helper.ths"]);
    let sources = [directory.path().join("src/bot.ths"), directory.path().join("src/lib/helper.ths")];
    assert_eq!(relative(directory.path(), project.stripped_paths(&sources)), ["build/bot.hs", "build/lib/helper.hs"]);
}

#[test]
fn output_keeps_paths_below_the_root_setting() {
    let (directory, project) = project("output = \"build\"\nroot = \".\"", &["src/bot.ths"]);
    let stripped = project.stripped_paths(&[directory.path().join("src/bot.ths")]);
    assert_eq!(relative(directory.path(), stripped), ["build/src/bot.hs"]);
}
