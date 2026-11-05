//! The checker's options a project's `[check]` settings turn on.

use ths_check::CheckOptions;
use ths_project::CheckSettings;

/// The options `settings` ask for.
pub fn options_from_settings(settings: &CheckSettings) -> CheckOptions {
    CheckOptions { strict: settings.strict, shadowing: settings.shadowing }
}
