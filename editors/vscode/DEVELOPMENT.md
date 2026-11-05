How to build, test and release the VSCode extension. What it does for its users is in [README.md](README.md), which is also its marketplace page.

## Building

```sh
pnpm install
pnpm run build      # bundles src/ into out/extension.js
pnpm test           # type-checks, then runs the unit tests and the grammar tests
pnpm run package    # writes typed-haasscript-<version>.vsix, without ths
pnpm run package:platform linux-x64   # writes typed-haasscript-linux-x64-<version>.vsix, with ths
```

`package:platform` takes a `vsce package --target` platform: `win32-x64`, `win32-arm64`, `darwin-x64`, `darwin-arm64`, `linux-x64`, `linux-arm64`, `alpine-x64` or `alpine-arm64`. It builds `ths` for the matching Rust target, so that target must be installed with `rustup target add` and able to link on the machine; Linux builds against musl, so the binary runs on every distribution. Building each platform on its own operating system is simplest. The marketplaces give each user the package for their platform, and the one without `ths` to everyone else.

The extension runs the `ths` the server path setting names, else the one in its `server/` directory, else `ths` on `PATH`. To work on the extension with your own build, set the server path to `target/release/ths` (or `target/debug/ths`).

## Testing

- The unit tests in `test/unit/` cover the code that needs no editor, such as finding a script's `ths.toml`. Node runs them as they are, without compiling, so the modules they import cannot use `enum` or other TypeScript that needs more than its types removed.
- The grammar tests in `test/grammar/` are written by `scripts/write-grammar-tests.mjs`; change the cases there and run `pnpm run test:grammar:write`.

## Trying it

Open this directory in VSCode and press F5, or install a `.vsix` with **Extensions: Install from VSIX…**.

## Releasing

Set `version` in `package.json`, commit, and push a tag of that version with a `v` in front, such as `v0.2.0`. The **Release** workflow in `.github/workflows/release.yml` then builds a package for each platform on that platform, plus the one without `ths`, and attaches them all to a GitHub release. It refuses a tag that does not match the version.

While `preview` is `true` in `package.json`, a release is an alpha: the packages are pre-releases, the marketplaces show a Preview badge and update only users who opt into pre-releases, and the GitHub release is a pre-release. Remove `preview` for the first stable release. The Visual Studio Marketplace does not accept versions like `0.2.0-alpha`, so a pre-release is told apart by this flag, not by its version.

The same workflow publishes to the Visual Studio Marketplace and to Open VSX, from the repository's `marketplaces` environment:

- The Visual Studio Marketplace uses Trusted Publishing, so no token is stored: the publisher's Trusted Publishing policy names this repository, the workflow file `release.yml` and the `marketplaces` environment. Once that policy exists, set the environment variable `VSCODE_MARKETPLACE_TRUSTED_PUBLISHING` to `true`; until then the workflow skips the Marketplace.
- Open VSX uses an access token, kept as the environment's secret `OVSX_PAT`; without it the workflow skips Open VSX.

To hold publishing until someone approves it, add a required reviewer to that environment.
