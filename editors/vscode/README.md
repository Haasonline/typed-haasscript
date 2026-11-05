Catch mistakes in your HaasOnline TradeServer scripts before you upload them. Typed HaasScript adds TypeScript-style types to HaasScript, and this extension checks your scripts as you type.

> **Alpha.** This is an early release, and you are one of its first users. If something goes wrong, or a correct script is reported as an error, please [open an issue](https://github.com/Haasonline/typed-haasscript/issues) saying which editor and operating system you use and, if you can, the lines that show it.

- **Errors as you type**, in `.ths` Typed HaasScript files and plain `.hs` HaasScript files: misspelled commands, wrong argument types, a number where text belongs, and more. A script sees the unsaved changes in your other open files.
- **Hover** over a name to see its type and the comment above where it is declared, including for every built-in HaasScript command.
- **Go to definition**, into the file a name is imported from.
- **Completion** of the names you can use, and of fields and methods after a `.` or `:`.
- **Syntax highlighting** for both kinds of file, with type annotations told apart from the script.
- Comment toggling, bracket matching and indentation for `function`, `then`, `do` and `repeat` blocks.

## Getting started

Scripts are checked when their folder, or a folder above it, holds a `ths.toml` settings file. When you open a script in a folder without one, the extension offers to create it: choose **Create ths.toml**, and checking starts at once. If you chose **Don't Ask Again**, run **Typed HaasScript: Create ths.toml** from the Command Palette instead.

The new `ths.toml` lists every setting, switched off, with a line about each. It works as it is; change it only if you want stricter checks or to choose which files are checked.

Nothing else needs installing on Windows, macOS or Linux: the extension comes with the `ths` checker inside it.

## Commands

- **Typed HaasScript: Create ths.toml** makes the open folder a project, so its scripts are checked.
- **Typed HaasScript: Restart the Language Server** starts the checker again, for example after updating `ths`. Changing the server path restarts it too.

## Settings

- **Typed HaasScript: Server Path** (`typedHaasScript.server.path`): the `ths` command to run instead of the one that comes with the extension. Leave it empty unless you build `ths` yourself or use a platform the extension carries no `ths` for, where it runs `ths` from your `PATH`.

`.lua` files are left to your editor's Lua support, so they are not checked here.

## Building the extension

See [DEVELOPMENT.md](DEVELOPMENT.md).
