/*
    Things this doesn't currently support that it might need to later:

    - Import parsing is unfinished and so is currently disabled
    - When import parsing is enabled:
        - Import renames (use a::b as c) - these are silently ignored
        - Imports that start with two colons (use ::a::b) - these are also silently ignored
*/

use std::{collections::HashMap, fmt::Debug, fs, path::PathBuf};

use cargo_metadata::MetadataCommand;
use log::{debug, warn};
use syn::{Attribute, Ident, ItemEnum, ItemStruct, UseTree};

use crate::markers;

/// Represents a crate, including a map of its modules, imports, structs and
/// enums.
#[derive(Debug, Clone)]
pub struct Crate {
    pub name: String,
    pub manifest_path: PathBuf,
    pub root_src_file: PathBuf,
    pub root_module: Module,
}

impl Crate {
    pub fn new(manifest_path: &str) -> Self {
        let mut cmd = MetadataCommand::new();
        cmd.manifest_path(&manifest_path);

        let metadata = cmd.exec().unwrap();

        let root_package = metadata.root_package().unwrap();
        let root_src_file = {
            let lib_file = root_package
                .manifest_path
                .parent()
                .unwrap()
                .join("src/lib.rs");
            let main_file = root_package
                .manifest_path
                .parent()
                .unwrap()
                .join("src/main.rs");

            if lib_file.exists() {
                fs::canonicalize(lib_file).unwrap()
            } else if main_file.exists() {
                fs::canonicalize(main_file).unwrap()
            } else {
                panic!("No src/lib.rs or src/main.rs found for this Cargo.toml file");
            }
        };

        let source_rust_content = fs::read_to_string(&root_src_file).unwrap();
        let file_ast = syn::parse_file(&source_rust_content).unwrap();

        let mut result = Crate {
            name: root_package.name.clone(),
            manifest_path: fs::canonicalize(manifest_path).unwrap(),
            root_src_file: root_src_file.clone(),
            root_module: Module {
                visibility: Visibility::Public,
                file_path: root_src_file,
                module_path: vec!["crate".to_string()],
                source: Some(ModuleSource::File(file_ast)),
                scope: None,
            },
        };

        result.resolve();

        result
    }

    /// Create a map of the modules for this crate
    pub fn resolve(&mut self) {
        self.root_module.resolve();
    }
}

/// Mirrors syn::Visibility, but can be created without a token
#[derive(Debug, Clone)]
pub enum Visibility {
    Public,
    Crate,
    Restricted, // Not supported
    Inherited,  // Usually means private
}

fn syn_vis_to_visibility(vis: &syn::Visibility) -> Visibility {
    match vis {
        syn::Visibility::Public(_) => Visibility::Public,
        syn::Visibility::Crate(_) => Visibility::Crate,
        syn::Visibility::Restricted(_) => Visibility::Restricted,
        syn::Visibility::Inherited => Visibility::Inherited,
    }
}

#[derive(Debug, Clone)]
pub struct Import {
    pub path: Vec<String>,
    pub visibility: Visibility,
}

#[derive(Debug, Clone)]
pub enum ModuleSource {
    File(syn::File),
    ModuleInFile(Vec<syn::Item>),
}

#[derive(Clone)]
pub struct Struct {
    pub ident: Ident,
    pub src: ItemStruct,
    pub visibility: Visibility,
    pub path: Vec<String>,
    pub mirror: bool,
}

impl Debug for Struct {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Struct")
            .field("ident", &self.ident)
            .field("src", &"omitted")
            .field("visibility", &self.visibility)
            .field("path", &self.path)
            .field("mirror", &self.mirror)
            .finish()
    }
}

#[derive(Clone)]
pub struct Enum {
    pub ident: Ident,
    pub src: ItemEnum,
    pub visibility: Visibility,
    pub path: Vec<String>,
    pub mirror: bool,
}

impl Debug for Enum {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Enum")
            .field("ident", &self.ident)
            .field("src", &"omitted")
            .field("visibility", &self.visibility)
            .field("path", &self.path)
            .field("mirror", &self.mirror)
            .finish()
    }
}

#[derive(Debug, Clone)]
pub struct ModuleScope {
    pub modules: Vec<Module>,
    pub enums: Vec<Enum>,
    pub structs: Vec<Struct>,
    pub imports: Vec<Import>,
}

#[derive(Clone)]
pub struct Module {
    pub visibility: Visibility,
    pub file_path: PathBuf,
    pub module_path: Vec<String>,
    pub source: Option<ModuleSource>,
    pub scope: Option<ModuleScope>,
}

impl Debug for Module {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Module")
            .field("visibility", &self.visibility)
            .field("module_path", &self.module_path)
            .field("file_path", &self.file_path)
            .field("source", &"omitted")
            .field("scope", &self.scope)
            .finish()
    }
}

/// Get a struct or enum ident, possibly remapped by a mirror marker
fn get_ident(ident: &Ident, attrs: &[Attribute]) -> (Ident, bool) {
    markers::extract_mirror_marker(attrs)
        .and_then(|path| path.get_ident().map(|ident| (ident.clone(), true)))
        .unwrap_or_else(|| (ident.clone(), false))
}

impl Module {
    pub fn resolve(&mut self) {
        self.resolve_modules();
        // self.resolve_imports();
    }

    /// Maps out modules, structs and enums within the scope of this module
    fn resolve_modules(&mut self) {
        let mut scope_modules = Vec::new();
        let mut scope_structs = Vec::new();
        let mut scope_enums = Vec::new();

        let items = match self.source.as_ref().unwrap() {
            ModuleSource::File(file) => &file.items,
            ModuleSource::ModuleInFile(items) => items,
        };

        for item in items.iter() {
            match item {
                syn::Item::Struct(item_struct) => {
                    let (ident, mirror) = get_ident(&item_struct.ident, &item_struct.attrs);
                    let ident_str = ident.to_string();
                    scope_structs.push(Struct {
                        ident,
                        src: item_struct.clone(),
                        visibility: syn_vis_to_visibility(&item_struct.vis),
                        path: {
                            let mut path = self.module_path.clone();
                            path.push(ident_str);
                            path
                        },
                        mirror,
                    });
                }
                syn::Item::Enum(item_enum) => {
                    let (ident, mirror) = get_ident(&item_enum.ident, &item_enum.attrs);
                    let ident_str = ident.to_string();
                    scope_enums.push(Enum {
                        ident,
                        src: item_enum.clone(),
                        visibility: syn_vis_to_visibility(&item_enum.vis),
                        path: {
                            let mut path = self.module_path.clone();
                            path.push(ident_str);
                            path
                        },
                        mirror,
                    });
                }
                syn::Item::Mod(item_mod) => {
                    let ident = item_mod.ident.clone();

                    let mut module_path = self.module_path.clone();
                    module_path.push(ident.to_string());

                    scope_modules.push(match &item_mod.content {
                        Some(content) => {
                            let mut child_module = Module {
                                visibility: syn_vis_to_visibility(&item_mod.vis),
                                file_path: self.file_path.clone(),
                                module_path,
                                source: Some(ModuleSource::ModuleInFile(content.1.clone())),
                                scope: None,
                            };

                            child_module.resolve();

                            child_module
                        }
                        None => {
                            let folder_path =
                                self.file_path.parent().unwrap().join(ident.to_string());
                            let folder_exists = folder_path.exists();

                            let file_path = if folder_exists {
                                folder_path.join("mod.rs")
                            } else {
                                self.file_path
                                    .parent()
                                    .unwrap()
                                    .join(ident.to_string() + ".rs")
                            };

                            let file_exists = file_path.exists();

                            if !file_exists {
                                warn!(
                                    "Skipping unresolvable module {} (tried {})",
                                    &ident,
                                    file_path.to_string_lossy()
                                );
                                continue;
                            }

                            let source = if file_exists {
                                let source_rust_content = fs::read_to_string(&file_path).unwrap();
                                debug!("Trying to parse {:?}", file_path);
                                Some(ModuleSource::File(
                                    syn::parse_file(&source_rust_content).unwrap(),
                                ))
                            } else {
                                None
                            };

                            let mut child_module = Module {
                                visibility: syn_vis_to_visibility(&item_mod.vis),
                                file_path,
                                module_path,
                                source,
                                scope: None,
                            };

                            if file_exists {
                                child_module.resolve();
                            }

                            child_module
                        }
                    });
                }
                _ => {}
            }
        }

        self.scope = Some(ModuleScope {
            modules: scope_modules,
            enums: scope_enums,
            structs: scope_structs,
            imports: vec![], // Will be filled in by resolve_imports()
        });
    }

    #[allow(dead_code)]
    fn resolve_imports(&mut self) {
        let imports = &mut self.scope.as_mut().unwrap().imports;

        let items = match self.source.as_ref().unwrap() {
            ModuleSource::File(file) => &file.items,
            ModuleSource::ModuleInFile(items) => items,
        };

        for item in items.iter() {
            if let syn::Item::Use(item_use) = item {
                let flattened_imports = flatten_use_tree(&item_use.tree);

                for import in flattened_imports {
                    imports.push(Import {
                        path: import,
                        visibility: syn_vis_to_visibility(&item_use.vis),
                    });
                }
            }
        }
    }

    pub fn collect_structs<'a>(&'a self, container: &mut HashMap<String, &'a Struct>) {
        let scope = self.scope.as_ref().unwrap();
        for scope_struct in &scope.structs {
            container.insert(scope_struct.ident.to_string(), scope_struct);
        }
        for scope_module in &scope.modules {
            scope_module.collect_structs(container);
        }
    }

    pub fn collect_structs_to_vec(&self) -> HashMap<String, &Struct> {
        let mut ans = HashMap::new();
        self.collect_structs(&mut ans);
        ans
    }

    pub fn collect_enums<'a>(&'a self, container: &mut HashMap<String, &'a Enum>) {
        let scope = self.scope.as_ref().unwrap();
        for scope_enum in &scope.enums {
            container.insert(scope_enum.ident.to_string(), scope_enum);
        }
        for scope_module in &scope.modules {
            scope_module.collect_enums(container);
        }
    }

    pub fn collect_enums_to_vec(&self) -> HashMap<String, &Enum> {
        let mut ans = HashMap::new();
        self.collect_enums(&mut ans);
        ans
    }
}

fn flatten_use_tree_rename_abort_warning(use_tree: &UseTree) {
    debug!("WARNING: flatten_use_tree() found an import rename (use a::b as c). flatten_use_tree() will now abort.");
    debug!("WARNING: This happened while parsing {:?}", use_tree);
    debug!("WARNING: This use statement will be ignored.");
}

/// Takes a use tree and returns a flat list of use paths (list of string tokens)
///
/// Example:
///     use a::{b::c, d::e};
/// becomes
///     [
///         ["a", "b", "c"],
///         ["a", "d", "e"]
///     ]
///
/// Warning: As of writing, import renames (import a::b as c) are silently
/// ignored.
fn flatten_use_tree(use_tree: &UseTree) -> Vec<Vec<String>> {
    // Vec<(path, is_complete)>
    let mut result = vec![(vec![], false)];

    let mut counter: usize = 0;

    loop {
        counter += 1;

        if counter > 10000 {
            panic!("flatten_use_tree: Use statement complexity limit exceeded. This is probably a bug.");
        }

        // If all paths are complete, break from the loop
        if result.iter().all(|result_item| result_item.1) {
            break;
        }

        let mut items_to_push = Vec::new();

        for path_tuple in &mut result {
            let path = &mut path_tuple.0;
            let is_complete = &mut path_tuple.1;

            if *is_complete {
                continue;
            }

            let mut tree_cursor = use_tree;

            for path_item in path.iter() {
                match tree_cursor {
                    UseTree::Path(use_path) => {
                        let ident = use_path.ident.to_string();
                        if *path_item != ident {
                            panic!("This ident did not match the one we already collected. This is a bug.");
                        }
                        tree_cursor = use_path.tree.as_ref();
                    }
                    UseTree::Group(use_group) => {
                        let mut moved_tree_cursor = false;

                        for tree in use_group.items.iter() {
                            match tree {
                                UseTree::Path(use_path) => {
                                    if path_item == &use_path.ident.to_string() {
                                        tree_cursor = use_path.tree.as_ref();
                                        moved_tree_cursor = true;
                                        break;
                                    }
                                }
                                // Since we're not matching UseTree::Group here, a::b::{{c}, {d}} might
                                // break. But also why would anybody do that
                                _ => unreachable!(),
                            }
                        }

                        if !moved_tree_cursor {
                            unreachable!();
                        }
                    }
                    _ => unreachable!(),
                }
            }

            match tree_cursor {
                UseTree::Name(use_name) => {
                    path.push(use_name.ident.to_string());
                    *is_complete = true;
                }
                UseTree::Path(use_path) => {
                    path.push(use_path.ident.to_string());
                }
                UseTree::Glob(_) => {
                    path.push("*".to_string());
                    *is_complete = true;
                }
                UseTree::Group(use_group) => {
                    // We'll modify the first one in-place, and make clones for
                    // all subsequent ones
                    let mut first: bool = true;
                    // Capture the path in this state, since we're about to
                    // modify it
                    let path_copy = path.clone();
                    for tree in use_group.items.iter() {
                        let mut new_path_tuple = if first {
                            None
                        } else {
                            let new_path = path_copy.clone();
                            items_to_push.push((new_path, false));
                            Some(items_to_push.iter_mut().last().unwrap())
                        };

                        match tree {
                            UseTree::Path(use_path) => {
                                let ident = use_path.ident.to_string();

                                if first {
                                    path.push(ident);
                                } else {
                                    new_path_tuple.unwrap().0.push(ident);
                                }
                            }
                            UseTree::Name(use_name) => {
                                let ident = use_name.ident.to_string();

                                if first {
                                    path.push(ident);
                                    *is_complete = true;
                                } else {
                                    let path_tuple = new_path_tuple.as_mut().unwrap();
                                    path_tuple.0.push(ident);
                                    path_tuple.1 = true;
                                }
                            }
                            UseTree::Glob(_) => {
                                if first {
                                    path.push("*".to_string());
                                    *is_complete = true;
                                } else {
                                    let path_tuple = new_path_tuple.as_mut().unwrap();
                                    path_tuple.0.push("*".to_string());
                                    path_tuple.1 = true;
                                }
                            }
                            UseTree::Group(_) => {
                                panic!(
                                    "Directly-nested use groups ({}) are not supported by flutter_rust_bridge. Use {} instead.",
                                    "use a::{{b}, c}",
                                    "a::{b, c}"
                                );
                            }
                            // UseTree::Group(_) => panic!(),
                            UseTree::Rename(_) => {
                                flatten_use_tree_rename_abort_warning(use_tree);
                                return vec![];
                            }
                        }

                        first = false;
                    }
                }
                UseTree::Rename(_) => {
                    flatten_use_tree_rename_abort_warning(use_tree);
                    return vec![];
                }
            }
        }

        for item in items_to_push {
            result.push(item);
        }
    }

    result.into_iter().map(|val| val.0).collect()
}
