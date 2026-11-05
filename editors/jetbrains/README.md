A plugin for IntelliJ-based IDEs (PyCharm, IntelliJ IDEA, WebStorm, RustRover and the rest), version 2026.2 or later, bringing Typed HaasScript support.

> **Alpha.** This is an early release, published to the JetBrains Marketplace's `alpha` channel. To install it, add `https://plugins.jetbrains.com/plugins/alpha/list` under **Settings | Plugins | ⚙ | Manage Plugin Repositories…**, then search for Typed HaasScript. If something goes wrong, or a correct script is reported as an error, please [open an issue](https://github.com/Haasonline/typed-haasscript/issues) saying which IDE and operating system you use and, if you can, the lines that show it.

- **Type errors as you type**, from `ths server`, for `.ths` files and plain `.hs` HaasScript files, through the IDE's built-in Language Server Protocol support. Each file is checked as `ths check` checks it, from the nearest `ths.toml` above it.
- **Hover and go-to-definition**: a name's declaration, type and documentation comment, and the way to where it is declared, into the module it is imported from.
- **Completion** of the names in scope, and of fields and methods after a `.` or `:`.
- **Syntax highlighting** from the same TextMate grammars as the VSCode extension, which the build copies from `../vscode`, so both editors highlight alike.

Every IntelliJ-based IDE has had the Language Server Protocol API since 2025.3, the free tier included; the plugin uses the client form of it that 2026.2 introduced, which replaces the earlier one.

## Requirements

The plugin runs the `ths` command, which is not bundled yet. Build it from the repository root with `cargo build --release` and put `target/release/ths` on your `PATH`, or set where it is in **Settings | Tools | Typed HaasScript**. Changing it there restarts the server.

Scripts are checked only in a folder that has a `ths.toml` file, in that folder or any folder above it. Without one, the plugin shows no errors, hover or completion for the script. When you open a script in a project that has none, the plugin offers to create one in the project folder:

- **Create ths.toml** writes it, with `ths init`, and checking starts.
- **Don't Ask Again** stops the offer for this project.
- Closing the message asks again next time you open the project.

You can create it at any time with **Tools | Create ths.toml** (or search for "Create ths.toml" with **Find Action**). If the project folder already has one, the plugin says so and leaves it alone.

`.lua` files are left to the IDE's Lua support, so they are not checked here; `ths check` still checks them.

## Building

The build needs Java 17 or later to run Gradle (an IDE's bundled runtime will do, through `JAVA_HOME`); it downloads the JDK 21 it compiles with and the PyCharm it builds against.

```sh
./gradlew buildPlugin    # writes build/distributions/typed-haasscript-<version>.zip
./gradlew test           # runs the tests in a headless PyCharm
./gradlew verifyPlugin   # checks the plugin against the PyCharm release it builds against and the newest one
./gradlew runIde         # opens a PyCharm with the plugin installed
```

Install the `.zip` with **Settings | Plugins | ⚙ | Install Plugin from Disk…**.

## Publishing

`./gradlew publishPlugin` uploads the plugin to the JetBrains Marketplace, with a token from the `JETBRAINS_MARKETPLACE_TOKEN` environment variable, to the channel `publishChannel` names in `gradle.properties`. It is `alpha` while early users try the plugin; set it to `default` for a stable release that every user sees.
