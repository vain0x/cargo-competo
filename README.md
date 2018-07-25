# competo

*No versions are availble yet*

`competo` is a command to copy-paste library code for competitive programming.

## Usage

`competo install foo` will include contents in `foo.rs` (or `foo/mod.rs`) into `main.rs`.

**Dependency Resolution**: If included mod contains use directives to import other mods, their contents are also patched into `main.rs`.

## Motivation

In competitive programming contests (such as AtCoder / Codeforces), solution must be a single file to submit under the limit of size.

**competo** helps to import reusable codes into solution by patching codes from multiple source files, excluding unncessary parts.

## Development Notes

### Issues

- Binaries don't work; `cargo run` works though
- Can't resolve dependency by function imports
- Duplicated use directives cause compile error
- Nav exprs will break
- Bad error handling
- Bad format of `macro_rules`
