use config;
use quote::ToTokens;
use std::collections::BTreeSet;
use std::env;
use std::fs;
use std::path::PathBuf;
use syn;

/// Finds project root directory from the current directory.
pub fn project_root_path() -> Option<PathBuf> {
    env::current_dir().ok().and_then(|mut cwd| loop {
        trace!("Search project from {}", cwd.display());
        cwd.push("Cargo.toml");
        if fs::metadata(cwd.as_path())
            .map(|meta| meta.is_file())
            .unwrap_or(false)
        {
            cwd.pop();
            return Some(cwd);
        }

        cwd.pop();
        if !cwd.pop() {
            return None;
        }
    })
}

fn format_src(src: &str) -> Option<String> {
    use rustfmt_nightly::{format_input, Config, EmitMode, Input, Verbosity};

    let mut rustfmt_config = Config::default();
    rustfmt_config.set().emit_mode(EmitMode::Stdout);
    rustfmt_config.set().verbose(Verbosity::Quiet);

    let mut out = Vec::with_capacity(src.len() * 2);
    let input = Input::Text(src.into());
    format_input(input, &rustfmt_config, Some(&mut out)).ok()?;
    String::from_utf8(out).ok()
}

type ModPathBuf = Vec<String>;

pub struct Source {
    syn_file: syn::File,
    uses: Vec<ModPathBuf>,
}

#[allow(unused)]
pub struct Entry {
    mod_name: String,
    mod_path: ModPathBuf,
    file_path: PathBuf,
    source: Option<Source>,
}

fn is_use_std(tree: &syn::UseTree) -> bool {
    match tree {
        syn::UseTree::Path(path) => path.ident == "std",
        syn::UseTree::Name(name) => name.ident == "std",
        syn::UseTree::Rename(rename) => rename.ident == "std",
        syn::UseTree::Group(group) => group.items.iter().any(|tree| is_use_std(tree)),
        _ => false,
    }
}

/// Collects paths from root to leaf of a use tree.
/// All paths prepend the specified path.
fn use_paths(item_use: &syn::ItemUse, path: &ModPathBuf, paths: &mut Vec<ModPathBuf>) {
    fn go(node: &syn::UseTree, buf: &mut ModPathBuf, paths: &mut Vec<ModPathBuf>) {
        match node {
            syn::UseTree::Path(path) => {
                buf.push(path.ident.to_string());
                go(&path.tree, buf, paths);
                buf.pop();
            }
            syn::UseTree::Name(name) => {
                let ident = name.ident.to_string();
                if ident == "self" {
                    paths.push(buf.to_owned());
                } else if ident == "super" {
                    let last = buf.pop().unwrap();
                    paths.push(buf.to_owned());
                    buf.push(last);
                }
                buf.push(name.ident.to_string());
                paths.push(buf.to_owned());
                buf.pop();
            }
            syn::UseTree::Rename(rename) => {
                // Ignore alias
                buf.push(rename.ident.to_string());
                paths.push(buf.to_owned());
                buf.pop();
            }
            syn::UseTree::Glob(_) => {
                // At '*' in foo::bar::*
                // buf = [foo, bar]
                // Assume all items defined in the mod are dependend on.
                // Just add path to the mod.
                paths.push(buf.to_owned());
            }
            syn::UseTree::Group(group) => {
                // At '{}' in foo::bar::{a, b, c::d}
                for node in group.items.iter() {
                    go(node, buf, paths);
                }
            }
        }
    }

    let mut buf = path.to_owned();
    go(&item_use.tree, &mut buf, paths);
}

/// Loads a source file.
/// lib.rs, foo/mod.rs or foo.rs.
/// mod_path: qualifier to the mod ([] for crate root, [bar, foo] for foo/mod.rs or foo.rs).
pub fn load_mod_file(
    mod_name: String,
    mod_path: ModPathBuf,
    file_path: PathBuf,
    entries: &mut Vec<Entry>,
) {
    trace!("load mod {:?}", mod_path);

    let result = fs::read_to_string(&file_path);
    if result.is_err() {
        error!("{:?} {:?}", file_path, result);
        return;
    }

    let content = result.unwrap();
    let syn_file = syn::parse_file(&content).unwrap();

    let mut items = Vec::new();
    let mut uses = Vec::new();

    // Retain items except for extern-crate, extern "C" etc.
    for item in syn_file.items.iter() {
        match item {
            syn::Item::ExternCrate(item) => {
                trace!("Ignore {:?}", item);
            }
            syn::Item::ForeignMod(item) => {
                trace!("Ignore {:?}", item);
            }
            syn::Item::Mod(item) => {
                trace!("Ignore {:?}", item);
            }
            syn::Item::Use(item) if !is_use_std(&item.tree) => {
                // Self-crate use statement. Stripped from the output.
                // Declares in-crate dependencies.
                let mut path = mod_path.to_owned();
                path.pop();
                use_paths(&item, &path, &mut uses);
            }
            _ => {
                // Copy to output.
                items.push(item.clone());
            }
        }
    }

    let syn_file = syn::File { items, ..syn_file };
    let source = Some(Source { syn_file, uses });

    entries.push(Entry {
        mod_name,
        mod_path,
        file_path,
        source,
    })
}

/// Does something and get final Rust code.
pub fn collect(config: &config::Config) -> String {
    // Find source directory.
    let src_path = match config.src_path {
        Some(src_path) => PathBuf::from(src_path),
        None => {
            let root_path = project_root_path().expect("Cargo project not found");
            root_path.join("src")
        }
    };
    let src_path = src_path.canonicalize().unwrap();

    let main_file = match config.main_path {
        Some(main_path) => PathBuf::from(main_path),
        None => src_path.join("src"),
    };

    use glob::glob;

    let is_dir = fs::metadata(&src_path)
        .map(|meta| meta.is_dir())
        .unwrap_or(false);

    if !is_dir {
        panic!(format!("Given dir doesn't exist: {:?}", src_path.to_str()))
    }

    // Enumerate source file paths.

    let pat = src_path.join("**").join("*.rs").display().to_string();
    trace!("collecting {}", pat);

    let paths = glob(&pat).unwrap();
    let file_paths = paths
        .into_iter()
        .collect::<Result<Vec<PathBuf>, _>>()
        .unwrap();

    // Load each source file as an entry.

    let mut entries = Vec::new();

    for file_path in file_paths {
        let mut mod_path = ModPathBuf::new();
        let mut mod_name = String::new();
        {
            use std::path;

            let path = match file_path.strip_prefix(&src_path) {
                Ok(path) => path,
                Err(_) => continue,
            };

            // Build mod path from file path.

            for component in path.components() {
                match component {
                    path::Component::Prefix(_)
                    | path::Component::RootDir
                    | path::Component::CurDir => {}
                    path::Component::ParentDir => {
                        mod_path.pop();
                    }
                    path::Component::Normal(name) => {
                        let name = name.to_str().unwrap();
                        if name == "lib.rs" {
                            mod_name = "lib".to_owned();
                        } else if name == "mod.rs" {
                            mod_name = mod_path.last().unwrap().to_owned();
                        } else if name.ends_with(".rs") {
                            let name = &name[0..name.len() - 3];
                            mod_name = name.to_owned();
                            mod_path.push(mod_name.to_owned());
                        } else {
                            mod_name = name.to_owned();
                            mod_path.push(mod_name.to_owned());
                        }
                    }
                }
            }
        }

        load_mod_file(mod_name, mod_path, src_path.join(file_path), &mut entries);
    }

    // Append rust code into single string buffer
    // by DFS on dependency graph starting from installed mods.

    fn go(entry: &Entry, entries: &Vec<Entry>, done: &mut BTreeSet<Vec<String>>, buf: &mut String) {
        if !done.insert(entry.mod_path.to_owned()) {
            return;
        }

        if let Some(source) = &entry.source {
            for dep in source.uses.iter() {
                let mut dep = dep.to_owned();

                // Identifiers starting with uppercase aren't crate/mod.
                loop {
                    if dep
                        .last()
                        .and_then(|name| name.chars().next())
                        .map(|c| c.is_uppercase())
                        .unwrap_or(false)
                    {
                        dep.pop();
                        continue;
                    }
                    break;
                }
                trace!("{:?} depends on {:?}", entry.mod_path, dep);

                for dep_entry in entries.iter() {
                    if !starts_with(&dep, &dep_entry.mod_path) {
                        continue;
                    }
                    go(dep_entry, entries, done, buf);
                }
            }

            let code = source.syn_file.clone().into_token_stream().to_string();
            *buf += &code;
        }
    }

    let install_mod_names = config
        .install_mod_names
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    let mut buf = String::new();
    let mut done = BTreeSet::new();
    for entry in entries.iter() {
        if !install_mod_names.contains(&entry.mod_name) {
            continue;
        }
        go(entry, &entries, &mut done, &mut buf);
    }

    let generated = format_src(&buf).unwrap();

    // Update main file.

    let mut main_code = String::new();
    fs::read_to_string(&mut main_code).unwrap();

    "".to_string()
}

fn starts_with(prefix: &ModPathBuf, path: &ModPathBuf) -> bool {
    if prefix.len() > path.len() {
        return false;
    }

    for i in 0..prefix.len() {
        if prefix[i] != path[i] {
            return false;
        }
    }

    true
}
