use config;
use quote::ToTokens;
use std::collections::BTreeSet;
use std::env;
use std::fs;
use std::path::PathBuf;
use syn;

const MANIFEST_NAME: &'static str = "Cargo.toml";

/// Tries to find project root directory from the current directory.
fn find_project_root() -> Option<PathBuf> {
    let mut dir = env::current_dir().expect("Couldn't get current dir.");
    trace!("Search project from {}", dir.display());

    loop {
        if let Ok(ref meta) = fs::metadata(&dir.join(MANIFEST_NAME)) {
            if meta.is_file() {
                return Some(dir);
            }
        }

        if !dir.pop() {
            return None;
        }
    }
}

fn format_rust_code(code: &str) -> String {
    use rustfmt_nightly::{format_input, Config, EmitMode, Input, Verbosity};

    let mut config = Config::default();
    config.set().emit_mode(EmitMode::Stdout);
    config.set().verbose(Verbosity::Quiet);

    let mut out = Vec::new();
    let input = Input::Text(code.to_owned());
    format_input(input, &config, Some(&mut out)).unwrap();
    String::from_utf8(out).unwrap()
}

type ModPathBuf = Vec<String>;

pub struct Source {
    syn_file: syn::File,
    deps: Vec<ModPathBuf>,
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
    let mut deps = Vec::new();

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
                use_paths(&item, &path, &mut deps);
            }
            _ => {
                // Copy to output.
                items.push(item.clone());
            }
        }
    }

    let syn_file = syn::File { items, ..syn_file };
    let source = Some(Source { syn_file, deps });

    entries.push(Entry {
        mod_name,
        mod_path,
        file_path,
        source,
    })
}

/// Does something and get final Rust code.
pub fn run(config: &config::Config) {
    // Find source directory,
    // resolve main file path

    let src_path = match config.src_path {
        Some(ref src_path) => PathBuf::from(src_path),
        None => {
            let root_path = find_project_root().expect("Cargo project not found");
            root_path.join("src")
        }
    };
    let src_path = src_path.canonicalize().unwrap();
    trace!("src_path = {}", src_path.display());

    let is_dir = fs::metadata(&src_path)
        .map(|meta| meta.is_dir())
        .unwrap_or(false);

    if !is_dir {
        panic!(format!("Given dir doesn't exist: {:?}", src_path.to_str()))
    }

    let main_path = match config.main_path {
        Some(ref main_path) => PathBuf::from(main_path),
        None => src_path.join("main.rs"),
    };
    trace!("main_path = {}", main_path.display());

    // Enumerate source file paths.

    use glob::glob;

    let pat = src_path.join("**").join("*.rs").display().to_string();
    trace!("collecting {}", pat);

    let paths = glob(&pat).unwrap();
    let file_paths = paths
        .into_iter()
        .collect::<Result<Vec<PathBuf>, _>>()
        .unwrap();

    trace!(
        "found {} {:?}",
        file_paths.len(),
        file_paths.iter().map(|p| p.display()).collect::<Vec<_>>()
    );

    // Load main file,
    // find current entry mods

    let main_code = fs::read_to_string(&main_path).unwrap();

    // mod names to be included
    let mut entry_mods = {
        let install_pat = "// competo install ";
        if let Some(index) = main_code.find(install_pat) {
            let index = index + install_pat.as_bytes().len();
            let end = main_code[index..]
                .find('\n')
                .map(|i| i + index)
                .unwrap_or(main_code.as_bytes().len());
            let names = main_code[index..end]
                .split_whitespace()
                .map(|word| word.trim())
                .filter(|&word| word != ",")
                .map(|word| word.to_owned())
                .collect::<Vec<String>>();
            names
        } else {
            vec![]
        }
    };
    trace!("already installed mods: {:?}", entry_mods);
    trace!("install mods: {:?}", config.install_mod_names);

    for name in config.install_mod_names.iter() {
        entry_mods.push(name.to_owned())
    }
    entry_mods.sort();
    entry_mods.dedup();

    let main_span = {
        let end_marker = "// competo end\n";
        let first = main_code.find("// competo start");
        let end = main_code
            .rfind(end_marker)
            .map(|i| i + end_marker.as_bytes().len());
        match (first, end) {
            (Some(first), Some(end)) => {
                trace!("competo range found: {}-{}", first, end);
                (first, end)
            }
            _ => {
                let len = main_code.as_bytes().len();
                (len, len)
            }
        }
    };

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
            trace!("visit {:?}", entry.mod_path);
            for dep in source.deps.iter() {
                let mut dep = dep.to_owned();
                trace!("  dep = {:?}", dep);

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

    let mut buf = String::new();
    let mut done = BTreeSet::new();
    for entry in entries.iter() {
        if !entry_mods.contains(&entry.mod_name) {
            continue;
        }
        go(entry, &entries, &mut done, &mut buf);
    }

    let generated = format_rust_code(&buf);

    let subst = format!(
        "// competo start\n// competo install {}\n{}// competo end\n",
        entry_mods.join(" "),
        generated
    );

    // Update main file.

    let updated_main_code = {
        let (begin, end) = main_span;
        format!("{}{}{}", &main_code[0..begin], subst, &main_code[end..])
    };

    fs::write(&main_path, &updated_main_code).unwrap();
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
