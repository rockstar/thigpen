use std::io::{Read, Write};
use std::path::Path;

use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize)]
pub struct Project {
    manifest: cargo_toml::Manifest,
    path: Box<Path>,
    pub module: Module,
}

impl Project {
    /// Load a project from a path
    ///
    /// The path will look for a `Cargo.toml` file in the current directory
    /// or any parent directory, recursively.
    pub fn from_path(path: &std::path::Path) -> Result<Self, String> {
        let cargo_toml_path = {
            assert!(path.is_dir(), "Path is {}", path.to_str().unwrap());
            let mut path = path.to_path_buf();

            loop {
                path.push("Cargo.toml");
                if path.is_file() {
                    break;
                }
                if !(path.pop() && path.pop()) {
                    eprintln!(
                        "Could not find Cargo.toml in any parent of the current working directory"
                    );
                    std::process::exit(1);
                }
            }
            path
        };
        log::debug!("Found manifest at {:?}", cargo_toml_path);

        let project_path = {
            let mut project_path = cargo_toml_path.clone();
            project_path.pop();
            project_path
        };

        let manifest = match cargo_toml::Manifest::from_path(&cargo_toml_path) {
            Ok(manifest) => manifest,
            Err(err) => {
                eprintln!("Could not parse Cargo.toml - {err}");
                std::process::exit(1);
            }
        };
        log::debug!("Manifest: {:?}", manifest);

        if let Some(product) = &manifest.lib {
            log::debug!("Analyzing library as {}", product.name.as_ref().unwrap());
            let module = {
                let mut lib_path = project_path.clone();
                lib_path.push(product.clone().path.unwrap());
                Module::from_file_path(product.name.as_ref().unwrap(), lib_path.as_path())
            };

            Ok(Project {
                manifest,
                module,
                path: project_path.into(),
            })
        } else {
            eprintln!("thigpen does not (yet) support non-lib crates");
            std::process::exit(1);
        }
    }

    /// Load a project from the cache
    ///
    /// The path will look for the cache file in the current directory
    /// or any parent directory, recursively.
    pub fn from_cache(path: &std::path::Path) -> Result<Self, String> {
        let path = path.to_path_buf();
        let cache_path = {
            assert!(path.is_dir(), "Path is {}", path.to_str().unwrap());

            let mut path = path.to_path_buf();
            loop {
                path.push(".thigpen.cache");
                if path.is_file() {
                    break;
                }
                if !(path.pop() && path.pop()) {
                    eprintln!(
                        "Could not find thigpen cache file in any parent of the current working directory"
                    );
                    std::process::exit(1);
                }
            }
            path
        };

        let mut contents = String::new();
        let mut file = std::fs::File::open(cache_path).unwrap();
        file.read_to_string(&mut contents).unwrap();
        let cache = serde_json::from_str(&contents).unwrap();
        Ok(cache)
    }

    /// Create a cache file
    pub fn cache(&self) -> Result<(), std::io::Error> {
        let mut cache_path: std::path::PathBuf = self.path.clone().into_path_buf();
        cache_path.push(".thigpen.cache");

        let mut cache = std::fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(cache_path)?;
        cache.write_all(serde_json::to_string(&self).unwrap().as_bytes())?;
        Ok(())
    }

    /// Return the name of the crate
    pub fn name(&self) -> String {
        match &self.manifest.package {
            Some(package) => package.name.clone(),
            None => String::from(""),
        }
    }
}

#[derive(Debug, Deserialize, Serialize)]
pub enum PublicIdentifierType {
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
pub struct PublicIdentifier {
    r#type: PublicIdentifierType,
    pub name: String,
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

#[derive(Debug, Deserialize, Serialize)]
#[allow(dead_code)]
pub struct Module {
    pub name: String,
    pub usepath: String,
    // TODO: not public please
    pub path: Option<Box<std::path::Path>>,
    pub interface: Vec<PublicIdentifier>,
    pub children: Vec<Module>,
}

impl Module {
    fn from_file_path(name: &str, path: &Path) -> Self {
        assert_eq!(path.file_name(), Some(std::ffi::OsStr::new("lib.rs")));

        let contents = std::fs::read_to_string(path)
            .unwrap_or_else(|err| panic!("Could not read {}: {}", path.to_str().unwrap(), err));
        let parsed_file = syn::parse_file(&contents).expect("Unable to parse file.");

        let children: Vec<Module> = parsed_file
            .items
            .iter()
            .filter_map(|item| match item {
                syn::Item::Mod(inner) => {
                    let module = Module::from_usepath(
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
            usepath: name.into(),
            path: Some(path.into()),
            interface,
            children,
        }
    }

    fn from_usepath(usepath: &str, path: &Path) -> Self {
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
                                Some(Module::from_usepath(
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
        }
    }
}
