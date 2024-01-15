use std::io::Write as ioWrite;
use std::path::Path;
use std::{fmt::Write, io::Read};

use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize)]
pub struct Cache {
    pub name: String,
    pub root: Lib,
}

impl Cache {
    pub fn write(&self) -> Result<(), std::io::Error> {
        let mut crate_root = self.root.path.clone().into_path_buf();
        // XXX: rockstar (13 Jan 2023) - This is...not right. We need something more
        // permanent than just guessing about all of this.
        crate_root.pop();
        crate_root.pop();
        crate_root.push(".thigpen.cache");

        let mut cache = std::fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(crate_root)?;
        cache.write_all(serde_json::to_string(self).unwrap().as_bytes())?;
        Ok(())
    }

    pub fn read(crate_root: std::path::PathBuf) -> Result<Self, std::io::Error> {
        let mut crate_root = crate_root;
        // XXX: rockstar (13 Jan 2023) - This is...not right. We need something more
        // permanent than just guessing about all of this.
        crate_root.pop();
        crate_root.push(".thigpen.cache");

        let mut contents = String::new();
        let mut file = std::fs::File::open(crate_root)?;
        file.read_to_string(&mut contents).unwrap();
        let cache = serde_json::from_str(&contents).unwrap();
        Ok(cache)
    }
}

#[derive(Debug, Deserialize, Serialize)]
enum PublicIdentifierType {
    Use,
    Const,
    Enum,
    ExternCrate,
    Fn,
    Mod,
    Static,
    Struct,
    Trait,
    TraitAlias,
    Type,
    Union,
}

impl std::fmt::Display for PublicIdentifierType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PublicIdentifierType::Use => write!(f, "use"),
            PublicIdentifierType::Const => write!(f, "const"),
            PublicIdentifierType::Enum => write!(f, "enum"),
            PublicIdentifierType::ExternCrate => write!(f, "extern crate"),
            PublicIdentifierType::Fn => write!(f, "fn"),
            PublicIdentifierType::Mod => write!(f, "mod"),
            PublicIdentifierType::Static => write!(f, "static"),
            PublicIdentifierType::Struct => write!(f, "struct"),
            PublicIdentifierType::Trait => write!(f, "trait"),
            PublicIdentifierType::TraitAlias => write!(f, "trait alias"),
            PublicIdentifierType::Type => write!(f, "type"),
            PublicIdentifierType::Union => write!(f, "union"),
        }
    }
}

#[derive(Debug, Deserialize, Serialize)]
struct PublicIdentifier {
    r#type: PublicIdentifierType,
    name: String,
}

impl PublicIdentifier {
    fn from_use(value: &syn::UseTree) -> Vec<Self> {
        match value {
            syn::UseTree::Path(usepath) => PublicIdentifier::from_use(&usepath.tree),
            syn::UseTree::Name(name) => vec![Self {
                name: name.ident.to_string(),
                r#type: PublicIdentifierType::Use,
            }],
            syn::UseTree::Rename(rename) => vec![Self {
                name: rename.rename.to_string(),
                r#type: PublicIdentifierType::Use,
            }],
            syn::UseTree::Glob(_) => vec![Self {
                name: "*".into(),
                r#type: PublicIdentifierType::Use,
            }],
            syn::UseTree::Group(group) => group
                .items
                .iter()
                .flat_map(PublicIdentifier::from_use)
                .collect(),
        }
    }

    fn find_in_items(items: &[syn::Item]) -> Vec<Self> {
        items
            .iter()
            .filter_map(|item| match item {
                syn::Item::Const(item) => match &item.vis {
                    syn::Visibility::Public(_) | syn::Visibility::Restricted(_) => {
                        Some(vec![PublicIdentifier {
                            name: item.ident.to_string(),
                            r#type: PublicIdentifierType::Const,
                        }])
                    }
                    syn::Visibility::Inherited => None,
                },
                syn::Item::Enum(item) => match &item.vis {
                    syn::Visibility::Public(_) | syn::Visibility::Restricted(_) => {
                        Some(vec![PublicIdentifier {
                            name: item.ident.to_string(),
                            r#type: PublicIdentifierType::Enum,
                        }])
                    }
                    syn::Visibility::Inherited => None,
                },
                syn::Item::ExternCrate(item) => match &item.vis {
                    syn::Visibility::Public(_) | syn::Visibility::Restricted(_) => {
                        Some(vec![PublicIdentifier {
                            name: item.ident.to_string(),
                            r#type: PublicIdentifierType::ExternCrate,
                        }])
                    }
                    syn::Visibility::Inherited => None,
                },
                syn::Item::Fn(item) => match &item.vis {
                    syn::Visibility::Public(_) | syn::Visibility::Restricted(_) => {
                        Some(vec![PublicIdentifier {
                            name: item.sig.ident.to_string(),
                            r#type: PublicIdentifierType::Fn,
                        }])
                    }
                    syn::Visibility::Inherited => None,
                },
                syn::Item::ForeignMod(_) => None,
                syn::Item::Impl(_) => None,
                syn::Item::Macro(_) => None,
                syn::Item::Mod(item) => match &item.vis {
                    syn::Visibility::Public(_) | syn::Visibility::Restricted(_) => {
                        Some(vec![PublicIdentifier {
                            name: item.ident.to_string(),
                            r#type: PublicIdentifierType::Mod,
                        }])
                    }
                    syn::Visibility::Inherited => None,
                },
                syn::Item::Static(item) => match &item.vis {
                    syn::Visibility::Public(_) | syn::Visibility::Restricted(_) => {
                        Some(vec![PublicIdentifier {
                            name: item.ident.to_string(),
                            r#type: PublicIdentifierType::Static,
                        }])
                    }
                    syn::Visibility::Inherited => None,
                },
                syn::Item::Struct(item) => match &item.vis {
                    syn::Visibility::Public(_) | syn::Visibility::Restricted(_) => {
                        Some(vec![PublicIdentifier {
                            name: item.ident.to_string(),
                            r#type: PublicIdentifierType::Struct,
                        }])
                    }
                    syn::Visibility::Inherited => None,
                },
                syn::Item::Trait(item) => match &item.vis {
                    syn::Visibility::Public(_) | syn::Visibility::Restricted(_) => {
                        Some(vec![PublicIdentifier {
                            name: item.ident.to_string(),
                            r#type: PublicIdentifierType::Trait,
                        }])
                    }
                    syn::Visibility::Inherited => None,
                },
                syn::Item::TraitAlias(item) => match &item.vis {
                    syn::Visibility::Public(_) | syn::Visibility::Restricted(_) => {
                        Some(vec![PublicIdentifier {
                            name: item.ident.to_string(),
                            r#type: PublicIdentifierType::TraitAlias,
                        }])
                    }
                    syn::Visibility::Inherited => None,
                },
                syn::Item::Type(item) => match &item.vis {
                    syn::Visibility::Public(_) | syn::Visibility::Restricted(_) => {
                        Some(vec![PublicIdentifier {
                            name: item.ident.to_string(),
                            r#type: PublicIdentifierType::Type,
                        }])
                    }
                    syn::Visibility::Inherited => None,
                },
                syn::Item::Union(item) => match &item.vis {
                    syn::Visibility::Public(_) | syn::Visibility::Restricted(_) => {
                        Some(vec![PublicIdentifier {
                            name: item.ident.to_string(),
                            r#type: PublicIdentifierType::Union,
                        }])
                    }
                    syn::Visibility::Inherited => None,
                },
                syn::Item::Use(item) => match &item.vis {
                    syn::Visibility::Public(_) | syn::Visibility::Restricted(_) => {
                        Some(PublicIdentifier::from_use(&item.tree))
                    }
                    syn::Visibility::Inherited => None,
                },
                syn::Item::Verbatim(_) => None,
                _ => None,
            })
            .flatten()
            .collect()
    }
}

/// A root crate
#[derive(Debug, Deserialize, Serialize)]
#[allow(dead_code)]
pub struct Lib {
    name: String,
    path: Box<Path>,
    interface: Vec<PublicIdentifier>,
    modules: Vec<Module>,
}

impl Lib {
    pub fn from_path(name: &str, path: &Path) -> Self {
        assert_eq!(path.file_name(), Some(std::ffi::OsStr::new("lib.rs")));

        let contents = std::fs::read_to_string(path)
            .unwrap_or_else(|err| panic!("Could not read {}: {}", path.to_str().unwrap(), err));
        let parsed_file = syn::parse_file(&contents).expect("Unable to parse file.");

        let children: Vec<Module> = parsed_file
            .items
            .iter()
            .filter_map(|item| match item {
                syn::Item::Mod(inner) => {
                    let module = Module::from_path(
                        &format!("{}::{}", name, inner.ident),
                        path.parent().unwrap(),
                    );
                    Some(module)
                }
                _ => None,
            })
            .collect();

        let interface: Vec<PublicIdentifier> = PublicIdentifier::find_in_items(&parsed_file.items);

        Self {
            name: name.into(),
            path: path.into(),
            interface,
            modules: children,
        }
    }

    #[allow(dead_code)]
    fn create_dot(&self) -> String {
        let mut dot = String::new();
        writeln!(&mut dot, "graph graphname {{").unwrap();
        for module in self.modules.iter() {
            writeln!(&mut dot, "\"{}\" -- \"{}\"", self.name, module.usepath).unwrap();
            module.write_dot(&mut dot);
        }
        writeln!(&mut dot, "}}").unwrap();

        dot
    }

    pub fn create_mermaid(&self) -> String {
        let mut mermaid = String::new();
        write!(
            &mut mermaid,
            r#"---
title: {} entity diagram
---
erDiagram
"#,
            self.name
        )
        .unwrap();
        self.modules.iter().for_each(|module| {
            writeln!(
                &mut mermaid,
                "  \"{}\" ||--|{{ \"{}\" : \"\"",
                self.name, module.usepath
            )
            .unwrap();
            module.write_mermaid(&mut mermaid);
        });
        writeln!(&mut mermaid, "  \"{}\" {{", self.name).unwrap();
        self.interface.iter().for_each(|item| {
            writeln!(&mut mermaid, "    {} {}", item.r#type, item.name).unwrap();
        });
        writeln!(&mut mermaid, "  }}").unwrap();

        mermaid
    }
}

#[derive(Debug, Deserialize, Serialize)]
#[allow(dead_code)]
struct Module {
    name: String,
    usepath: String,
    path: Option<Box<Path>>,
    interface: Vec<PublicIdentifier>,
    children: Vec<Module>,
    dependencies: Vec<Module>,
}

impl Module {
    fn from_path(usepath: &str, path: &Path) -> Self {
        assert_ne!(path.file_name(), Some(std::ffi::OsStr::new("lib.rs")));
        assert_ne!(path.extension(), Some(std::ffi::OsStr::new("rs")));
        assert!(path.exists(), "Path at {:?} does not exist", path);
        assert!(path.is_dir(), "Path at {:?} is not a directory", path);

        let name = usepath.split("::").last().unwrap();

        let paths: Vec<std::path::PathBuf> = vec![
            {
                let mut path = path.to_path_buf();
                path.push(name);
                path.push("mod");
                path.set_extension("rs");
                path
            },
            {
                let mut path = path.to_path_buf();
                path.push(name);
                path.set_extension("rs");
                path
            },
        ]
        .into_iter()
        .filter(|path| path.exists())
        .collect();
        if paths.is_empty() {
            // Either the module lives inside the declaring file, or the module
            // doesn't exist on disk. Either way, drop a "dummy" module here.
            return Self {
                name: name.into(),
                usepath: usepath.into(),
                path: None,
                interface: vec![],
                children: vec![],
                dependencies: vec![],
            };
        }

        let (children, interface): (Vec<Module>, Vec<PublicIdentifier>) = paths
            .iter()
            .map(|modpath| {
                log::debug!("Reading/parsing {:?}", path);

                let contents = std::fs::read_to_string(modpath).unwrap_or_else(|err| {
                    panic!("Could not read {}: {}", modpath.to_str().unwrap(), err)
                });
                let parsed_file = syn::parse_file(&contents).expect("Unable to parse file.");
                let modules: Vec<Module> = parsed_file
                    .items
                    .iter()
                    .filter_map(|item| match item {
                        syn::Item::Mod(inner) => {
                            let mut path = path.to_path_buf();
                            path.push(name);
                            if path.exists() {
                                Some(Module::from_path(
                                    &format!("{}::{}", usepath, inner.ident),
                                    &path,
                                ))
                            } else {
                                None
                            }
                        }
                        syn::Item::Use(_inner) => None,
                        _ => None,
                    })
                    .collect();

                let interface: Vec<PublicIdentifier> =
                    PublicIdentifier::find_in_items(&parsed_file.items);

                (modules, interface)
            })
            .fold((vec![], vec![]), |mut acc, x| {
                acc.0.extend(x.0);
                acc.1.extend(x.1);
                acc
            });

        Self {
            name: name.into(),
            usepath: usepath.into(),
            path: Some(path.into()),
            interface,
            children,
            dependencies: vec![],
        }
    }

    fn write_dot(&self, mut wrt: &mut dyn std::fmt::Write) {
        for module in self.children.iter() {
            writeln!(&mut wrt, "\"{}\" -- \"{}\"", self.usepath, module.usepath).unwrap();
            module.write_dot(wrt);
        }
    }

    fn write_mermaid(&self, mut wrt: &mut dyn std::fmt::Write) {
        self.children.iter().for_each(|module| {
            writeln!(
                &mut wrt,
                "  \"{}\" ||--|{{ \"{}\" : \"\"",
                self.usepath, module.usepath
            )
            .unwrap();
            module.write_mermaid(&mut wrt);
        });

        writeln!(&mut wrt, "  \"{}\" {{", self.usepath).unwrap();
        self.interface.iter().for_each(|item| {
            writeln!(&mut wrt, "    {} {}", item.r#type, item.name).unwrap();
        });
        writeln!(&mut wrt, "  }}").unwrap();
    }
}
