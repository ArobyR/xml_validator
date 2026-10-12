# XML Schema Validator

A cross-platform desktop application that validates XML documents against XSD
schemas. It provides a simple graphical interface over `xmllint` (libxml2) and
classifies failures so you can tell immediately whether the problem is in your
XML, in your XSD, or in the environment.

## Features

- Validate an XML document against an XSD schema with one click
- Runs on a background thread: the UI never freezes, and hung validations are
  killed after 120 seconds
- Failure classification: schema violations, malformed XML, broken/missing XSD,
  and environment problems are shown as distinct results with explanations
- Full raw `xmllint` output is preserved under "Technical details"
- Automatic `xmllint` detection (PATH, Homebrew locations, system locations)
  with a manual "Locate xmllint..." fallback
- UI languages via translation files (English and Spanish included; more can be
  added without recompiling)

## Requirements

- A recent Rust toolchain ([rustup](https://rustup.rs))
- `xmllint` (part of libxml2)

### Installing xmllint

| Platform | Command |
| --- | --- |
| macOS (Homebrew) | `brew install libxml2` |
| macOS (built-in) | `/usr/bin/xmllint` is already present |
| Debian / Ubuntu | `sudo apt install libxml2-utils` |
| Fedora / RHEL | `sudo dnf install libxml2` |
| Windows (MSYS2) | `pacman -S mingw-w64-x86_64-libxml2` |

Note for Homebrew users: libxml2 is installed as keg-only. If the application
does not detect it automatically, use "Locate xmllint..." in the status bar and
select `/opt/homebrew/bin/xmllint`, or add `/opt/homebrew/bin` to your `PATH`.

## Building and running

```sh
cargo run --release
```

## Usage

1. Click **Browse...** next to "XML document" and select your XML file
2. Click **Browse...** next to "XSD schema" and select your XSD file
3. Click **Validate**

The result panel tells you whether the document is valid and, if not, at which
stage validation failed. The raw output of `xmllint` is available under
**Technical details**.

## Adding or changing translations

Translations are plain JSON files of key/value pairs in the `locales/`
directory. Two languages are bundled: `en.json` (English) and `es.json`
(Spanish).

To add a language, create `locales/<code>.json` (for example `fr.json`) using
`en.json` as a template, and set `_name` to the language's native name:

```json
{
  "_name": "Français",
  "app.title": "Validateur de schémas XML"
}
```

Bundled languages are compiled into the executable. Additionally, any
`locales/*.json` file placed next to the executable (or in the current working
directory) is loaded at startup — external files override bundled keys, and new
languages appear in the language selector without recompiling. Keys that are
missing from a translation fall back to English.

## Notes and limitations

- Validation is delegated to `xmllint`, so behavior matches libxml2's XSD 1.0
  support
- Large schema-violating documents may yield many errors; the raw output stays
  available even when the summary is long

## Continuous integration

The GitHub Actions workflow (`.github/workflows/ci.yml`) runs `cargo fmt`,
`cargo clippy -D warnings`, and `cargo test` on every push and pull request.

## License

This project is licensed under the GNU General Public License v3.0. See the
[LICENSE](LICENSE) file for the full text.
