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
        // XXX: rockstar (13 Oct 2025) - There's a serialization error here
        // where the cache is too old for current struct definition, so the
        // cache needs to be regenerated.
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

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct ModuleDependency {
    /// The module path that this module depends on (e.g., "crate::foo::bar")
    pub target_module: String,
    /// The symbols imported from that module via `use` statements
    pub imported_symbols: Vec<String>,
    /// The symbols referenced directly from that module (e.g., crate::foo::Symbol::new())
    pub referenced_symbols: Vec<String>,
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

    /// Extract the full path from a use tree (e.g., "crate::foo::bar")
    fn extract_use_paths(value: &syn::UseTree) -> Vec<String> {
        fn extract_path_recursive(tree: &syn::UseTree, current_path: String) -> Vec<String> {
            match tree {
                syn::UseTree::Path(usepath) => {
                    let segment = usepath.ident.to_string();
                    let new_path = if current_path.is_empty() {
                        segment
                    } else {
                        format!("{}::{}", current_path, segment)
                    };
                    extract_path_recursive(&usepath.tree, new_path)
                }
                syn::UseTree::Name(name) => {
                    let segment = name.ident.to_string();
                    let full_path = if current_path.is_empty() {
                        segment
                    } else {
                        format!("{}::{}", current_path, segment)
                    };
                    vec![full_path]
                }
                syn::UseTree::Rename(rename) => {
                    let segment = rename.ident.to_string();
                    let full_path = if current_path.is_empty() {
                        segment
                    } else {
                        format!("{}::{}", current_path, segment)
                    };
                    vec![full_path]
                }
                syn::UseTree::Glob(_) => {
                    // For glob imports, return the path to the module
                    vec![current_path]
                }
                syn::UseTree::Group(group) => group
                    .items
                    .iter()
                    .flat_map(|item| extract_path_recursive(item, current_path.clone()))
                    .collect(),
            }
        }
        extract_path_recursive(value, String::new())
    }

    /// Extract internal crate dependencies from syn items
    fn extract_dependencies_from_items(
        items: &[syn::Item],
        current_module_path: &str,
    ) -> Vec<ModuleDependency> {
        let mut dependencies = std::collections::HashMap::new();

        // First pass: extract use statements
        for item in items {
            if let syn::Item::Use(use_item) = item {
                let paths = PublicIdentifier::extract_use_paths(&use_item.tree);

                for path in paths {
                    // Only consider internal crate references
                    if path.starts_with("crate::")
                        || path.starts_with("super::")
                        || path.starts_with("self::")
                    {
                        let normalized_path =
                            PublicIdentifier::normalize_use_path(&path, current_module_path);

                        if let Some((module_path, symbol)) =
                            PublicIdentifier::split_module_and_symbol(&normalized_path)
                        {
                            dependencies
                                .entry(module_path)
                                .or_insert_with(|| (Vec::new(), Vec::new()))
                                .0
                                .push(symbol);
                        }
                    }
                }
            }
        }

        // Second pass: extract direct references
        for item in items {
            let direct_refs =
                PublicIdentifier::extract_direct_references(item, current_module_path);
            for (module_path, symbol) in direct_refs {
                dependencies
                    .entry(module_path)
                    .or_insert_with(|| (Vec::new(), Vec::new()))
                    .1
                    .push(symbol);
            }
        }

        dependencies
            .into_iter()
            .map(
                |(target_module, (imported_symbols, referenced_symbols))| ModuleDependency {
                    target_module,
                    imported_symbols,
                    referenced_symbols,
                },
            )
            .collect()
    }

    /// Normalize relative paths (super::, self::) to absolute crate paths
    fn normalize_use_path(path: &str, current_module_path: &str) -> String {
        if path.starts_with("crate::") {
            path.to_string()
        } else if path.starts_with("super::") {
            // Navigate up one level from current module
            let parts: Vec<&str> = current_module_path.split("::").collect();
            if parts.len() > 1 {
                let parent_path = parts[..parts.len() - 1].join("::");
                path.replacen("super::", &format!("{}::", parent_path), 1)
            } else {
                // super:: from root goes to crate::
                path.replacen("super::", "crate::", 1)
            }
        } else if path.starts_with("self::") {
            path.replacen("self::", &format!("{}::", current_module_path), 1)
        } else {
            // Relative path, assume it's within current module
            format!("{}::{}", current_module_path, path)
        }
    }

    /// Split a full path into module path and symbol name
    /// e.g., "crate::foo::bar::Symbol" -> Some(("crate::foo::bar", "Symbol"))
    /// For method calls like "crate::foo::Type::method", we want ("crate::foo", "Type")
    fn split_module_and_symbol(path: &str) -> Option<(String, String)> {
        let parts: Vec<&str> = path.split("::").collect();
        if parts.len() >= 2 {
            // For paths with multiple segments, we assume the last segment that starts
            // with a capital letter is the type, and everything after that is a method/field
            let mut type_index = parts.len() - 1;

            // Look for the last segment that starts with a capital letter (likely a type)
            for (i, part) in parts.iter().enumerate().rev() {
                if part
                    .chars()
                    .next()
                    .map(|c| c.is_uppercase())
                    .unwrap_or(false)
                {
                    type_index = i;
                    break;
                }
            }

            if type_index > 0 {
                let symbol = parts[type_index].to_string();
                let module_path = parts[..type_index].join("::");
                Some((module_path, symbol))
            } else {
                // Fallback to the old logic if we can't find a proper type
                let symbol = parts.last().unwrap().to_string();
                let module_path = parts[..parts.len() - 1].join("::");
                Some((module_path, symbol))
            }
        } else {
            None
        }
    }

    /// Extract direct references to internal crate symbols from an item
    /// This analyzes the entire AST of the item to find path expressions
    fn extract_direct_references(
        item: &syn::Item,
        current_module_path: &str,
    ) -> Vec<(String, String)> {
        let mut references = Vec::new();

        match item {
            syn::Item::Fn(item_fn) => {
                // Analyze function signature
                PublicIdentifier::visit_fn_sig(&item_fn.sig, current_module_path, &mut references);

                // Analyze function body
                PublicIdentifier::visit_block(&item_fn.block, current_module_path, &mut references);
            }
            syn::Item::Struct(item_struct) => {
                // Analyze struct fields
                match &item_struct.fields {
                    syn::Fields::Named(fields) => {
                        for field in &fields.named {
                            PublicIdentifier::visit_type(
                                &field.ty,
                                current_module_path,
                                &mut references,
                            );
                        }
                    }
                    syn::Fields::Unnamed(fields) => {
                        for field in &fields.unnamed {
                            PublicIdentifier::visit_type(
                                &field.ty,
                                current_module_path,
                                &mut references,
                            );
                        }
                    }
                    syn::Fields::Unit => {}
                }
            }
            syn::Item::Enum(item_enum) => {
                // Analyze enum variants
                for variant in &item_enum.variants {
                    match &variant.fields {
                        syn::Fields::Named(fields) => {
                            for field in &fields.named {
                                PublicIdentifier::visit_type(
                                    &field.ty,
                                    current_module_path,
                                    &mut references,
                                );
                            }
                        }
                        syn::Fields::Unnamed(fields) => {
                            for field in &fields.unnamed {
                                PublicIdentifier::visit_type(
                                    &field.ty,
                                    current_module_path,
                                    &mut references,
                                );
                            }
                        }
                        syn::Fields::Unit => {}
                    }
                }
            }
            syn::Item::Trait(item_trait) => {
                // Analyze trait items
                for trait_item in &item_trait.items {
                    match trait_item {
                        syn::TraitItem::Fn(trait_fn) => {
                            PublicIdentifier::visit_fn_sig(
                                &trait_fn.sig,
                                current_module_path,
                                &mut references,
                            );
                            if let Some(block) = &trait_fn.default {
                                PublicIdentifier::visit_block(
                                    block,
                                    current_module_path,
                                    &mut references,
                                );
                            }
                        }
                        syn::TraitItem::Type(trait_type) => {
                            if let Some(default) = &trait_type.default {
                                PublicIdentifier::visit_type(
                                    &default.1,
                                    current_module_path,
                                    &mut references,
                                );
                            }
                        }
                        syn::TraitItem::Const(trait_const) => {
                            PublicIdentifier::visit_type(
                                &trait_const.ty,
                                current_module_path,
                                &mut references,
                            );
                            if let Some(default) = &trait_const.default {
                                PublicIdentifier::visit_expr(
                                    &default.1,
                                    current_module_path,
                                    &mut references,
                                );
                            }
                        }
                        _ => {}
                    }
                }
            }
            syn::Item::Impl(item_impl) => {
                // Analyze impl block
                PublicIdentifier::visit_type(
                    &item_impl.self_ty,
                    current_module_path,
                    &mut references,
                );
                if let Some((_, trait_path, _)) = &item_impl.trait_ {
                    PublicIdentifier::visit_path(trait_path, current_module_path, &mut references);
                }

                for impl_item in &item_impl.items {
                    match impl_item {
                        syn::ImplItem::Fn(impl_fn) => {
                            PublicIdentifier::visit_fn_sig(
                                &impl_fn.sig,
                                current_module_path,
                                &mut references,
                            );
                            PublicIdentifier::visit_block(
                                &impl_fn.block,
                                current_module_path,
                                &mut references,
                            );
                        }
                        syn::ImplItem::Type(impl_type) => {
                            PublicIdentifier::visit_type(
                                &impl_type.ty,
                                current_module_path,
                                &mut references,
                            );
                        }
                        syn::ImplItem::Const(impl_const) => {
                            PublicIdentifier::visit_type(
                                &impl_const.ty,
                                current_module_path,
                                &mut references,
                            );
                            PublicIdentifier::visit_expr(
                                &impl_const.expr,
                                current_module_path,
                                &mut references,
                            );
                        }
                        _ => {}
                    }
                }
            }
            syn::Item::Type(item_type) => {
                PublicIdentifier::visit_type(&item_type.ty, current_module_path, &mut references);
            }
            syn::Item::Const(item_const) => {
                PublicIdentifier::visit_type(&item_const.ty, current_module_path, &mut references);
                PublicIdentifier::visit_expr(
                    &item_const.expr,
                    current_module_path,
                    &mut references,
                );
            }
            syn::Item::Static(item_static) => {
                PublicIdentifier::visit_type(&item_static.ty, current_module_path, &mut references);
                PublicIdentifier::visit_expr(
                    &item_static.expr,
                    current_module_path,
                    &mut references,
                );
            }
            _ => {
                // For other items (mods, uses, etc.), we don't need to analyze references
            }
        }

        references
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

    /// Visit a function signature to find type references
    fn visit_fn_sig(
        sig: &syn::Signature,
        current_module_path: &str,
        references: &mut Vec<(String, String)>,
    ) {
        // Visit input types
        for input in &sig.inputs {
            match input {
                syn::FnArg::Typed(pat_type) => {
                    PublicIdentifier::visit_type(&pat_type.ty, current_module_path, references);
                }
                syn::FnArg::Receiver(receiver) => {
                    // For self parameters, check if there's a type annotation
                    if let syn::Type::Reference(type_ref) = &*receiver.ty {
                        PublicIdentifier::visit_type(
                            &type_ref.elem,
                            current_module_path,
                            references,
                        );
                    } else {
                        PublicIdentifier::visit_type(&receiver.ty, current_module_path, references);
                    }
                }
            }
        }

        // Visit return type
        match &sig.output {
            syn::ReturnType::Type(_, ty) => {
                PublicIdentifier::visit_type(ty, current_module_path, references);
            }
            syn::ReturnType::Default => {}
        }
    }

    /// Visit a type to find path references
    fn visit_type(
        ty: &syn::Type,
        current_module_path: &str,
        references: &mut Vec<(String, String)>,
    ) {
        match ty {
            syn::Type::Path(type_path) => {
                PublicIdentifier::visit_path(&type_path.path, current_module_path, references);
            }
            syn::Type::Reference(type_ref) => {
                PublicIdentifier::visit_type(&type_ref.elem, current_module_path, references);
            }
            syn::Type::Ptr(type_ptr) => {
                PublicIdentifier::visit_type(&type_ptr.elem, current_module_path, references);
            }
            syn::Type::Slice(type_slice) => {
                PublicIdentifier::visit_type(&type_slice.elem, current_module_path, references);
            }
            syn::Type::Array(type_array) => {
                PublicIdentifier::visit_type(&type_array.elem, current_module_path, references);
                PublicIdentifier::visit_expr(&type_array.len, current_module_path, references);
            }
            syn::Type::Tuple(type_tuple) => {
                for elem in &type_tuple.elems {
                    PublicIdentifier::visit_type(elem, current_module_path, references);
                }
            }
            syn::Type::Paren(type_paren) => {
                PublicIdentifier::visit_type(&type_paren.elem, current_module_path, references);
            }
            syn::Type::Group(type_group) => {
                PublicIdentifier::visit_type(&type_group.elem, current_module_path, references);
            }
            _ => {
                // Other types don't contain paths we care about
            }
        }
    }

    /// Visit a path to extract internal crate references
    fn visit_path(
        path: &syn::Path,
        current_module_path: &str,
        references: &mut Vec<(String, String)>,
    ) {
        let path_str = path
            .segments
            .iter()
            .map(|segment| segment.ident.to_string())
            .collect::<Vec<_>>()
            .join("::");

        // Skip simple identifiers that don't represent module paths
        if !path_str.contains("::") {
            return;
        }

        // Only consider internal crate references, but filter out pure "self" references
        if (path_str.starts_with("crate")
            || path_str.starts_with("super")
            || path_str.starts_with("self::"))
            && path_str != "self"
        {
            let normalized_path =
                PublicIdentifier::normalize_use_path(&path_str, current_module_path);

            // Don't create self-references
            if let Some((module_path, symbol)) =
                PublicIdentifier::split_module_and_symbol(&normalized_path)
            {
                let current_module_normalized = current_module_path.replace("thigpen::", "crate::");
                if module_path != current_module_normalized && module_path != current_module_path {
                    references.push((module_path, symbol));
                }
            }
        }

        // Also visit any type arguments in the path segments
        for segment in &path.segments {
            match &segment.arguments {
                syn::PathArguments::AngleBracketed(args) => {
                    for arg in &args.args {
                        match arg {
                            syn::GenericArgument::Type(ty) => {
                                PublicIdentifier::visit_type(ty, current_module_path, references);
                            }
                            syn::GenericArgument::Const(expr) => {
                                PublicIdentifier::visit_expr(expr, current_module_path, references);
                            }
                            _ => {}
                        }
                    }
                }
                syn::PathArguments::Parenthesized(args) => {
                    for input in &args.inputs {
                        PublicIdentifier::visit_type(input, current_module_path, references);
                    }
                    if let syn::ReturnType::Type(_, ty) = &args.output {
                        PublicIdentifier::visit_type(ty, current_module_path, references);
                    }
                }
                syn::PathArguments::None => {}
            }
        }
    }

    /// Visit a block to find all references within
    fn visit_block(
        block: &syn::Block,
        current_module_path: &str,
        references: &mut Vec<(String, String)>,
    ) {
        for stmt in &block.stmts {
            PublicIdentifier::visit_stmt(stmt, current_module_path, references);
        }
    }

    /// Visit a statement
    fn visit_stmt(
        stmt: &syn::Stmt,
        current_module_path: &str,
        references: &mut Vec<(String, String)>,
    ) {
        match stmt {
            syn::Stmt::Local(local) => {
                if let Some(init) = &local.init {
                    PublicIdentifier::visit_expr(&init.expr, current_module_path, references);
                }
                // Extract type from pattern if it's a typed pattern
                PublicIdentifier::visit_pat(&local.pat, current_module_path, references);
            }
            syn::Stmt::Item(item) => {
                let item_refs =
                    PublicIdentifier::extract_direct_references(item, current_module_path);
                references.extend(item_refs);
            }
            syn::Stmt::Expr(expr, _) => {
                PublicIdentifier::visit_expr(expr, current_module_path, references);
            }
            syn::Stmt::Macro(stmt_macro) => {
                // For macro calls, we could analyze the tokens, but that's complex
                // For now, skip macro analysis
                let _ = stmt_macro;
            }
        }
    }

    /// Visit an expression to find all path references
    fn visit_expr(
        expr: &syn::Expr,
        current_module_path: &str,
        references: &mut Vec<(String, String)>,
    ) {
        match expr {
            syn::Expr::Path(expr_path) => {
                PublicIdentifier::visit_path(&expr_path.path, current_module_path, references);
            }
            syn::Expr::Call(expr_call) => {
                PublicIdentifier::visit_expr(&expr_call.func, current_module_path, references);
                for arg in &expr_call.args {
                    PublicIdentifier::visit_expr(arg, current_module_path, references);
                }
            }
            syn::Expr::MethodCall(expr_method) => {
                PublicIdentifier::visit_expr(
                    &expr_method.receiver,
                    current_module_path,
                    references,
                );
                for arg in &expr_method.args {
                    PublicIdentifier::visit_expr(arg, current_module_path, references);
                }
            }
            syn::Expr::Field(expr_field) => {
                PublicIdentifier::visit_expr(&expr_field.base, current_module_path, references);
            }
            syn::Expr::Index(expr_index) => {
                PublicIdentifier::visit_expr(&expr_index.expr, current_module_path, references);
                PublicIdentifier::visit_expr(&expr_index.index, current_module_path, references);
            }
            syn::Expr::Unary(expr_unary) => {
                PublicIdentifier::visit_expr(&expr_unary.expr, current_module_path, references);
            }
            syn::Expr::Binary(expr_binary) => {
                PublicIdentifier::visit_expr(&expr_binary.left, current_module_path, references);
                PublicIdentifier::visit_expr(&expr_binary.right, current_module_path, references);
            }
            syn::Expr::Cast(expr_cast) => {
                PublicIdentifier::visit_expr(&expr_cast.expr, current_module_path, references);
                PublicIdentifier::visit_type(&expr_cast.ty, current_module_path, references);
            }

            syn::Expr::Let(expr_let) => {
                PublicIdentifier::visit_expr(&expr_let.expr, current_module_path, references);
            }
            syn::Expr::If(expr_if) => {
                PublicIdentifier::visit_expr(&expr_if.cond, current_module_path, references);
                PublicIdentifier::visit_block(
                    &expr_if.then_branch,
                    current_module_path,
                    references,
                );
                if let Some((_, else_branch)) = &expr_if.else_branch {
                    PublicIdentifier::visit_expr(else_branch, current_module_path, references);
                }
            }
            syn::Expr::While(expr_while) => {
                PublicIdentifier::visit_expr(&expr_while.cond, current_module_path, references);
                PublicIdentifier::visit_block(&expr_while.body, current_module_path, references);
            }
            syn::Expr::ForLoop(expr_for) => {
                PublicIdentifier::visit_expr(&expr_for.expr, current_module_path, references);
                PublicIdentifier::visit_block(&expr_for.body, current_module_path, references);
            }
            syn::Expr::Loop(expr_loop) => {
                PublicIdentifier::visit_block(&expr_loop.body, current_module_path, references);
            }
            syn::Expr::Match(expr_match) => {
                PublicIdentifier::visit_expr(&expr_match.expr, current_module_path, references);
                for arm in &expr_match.arms {
                    if let Some((_, guard)) = &arm.guard {
                        PublicIdentifier::visit_expr(guard, current_module_path, references);
                    }
                    PublicIdentifier::visit_expr(&arm.body, current_module_path, references);
                }
            }
            syn::Expr::Closure(expr_closure) => {
                for input in &expr_closure.inputs {
                    if let syn::Pat::Type(pat_type) = input {
                        PublicIdentifier::visit_type(&pat_type.ty, current_module_path, references);
                    }
                }
                PublicIdentifier::visit_expr(&expr_closure.body, current_module_path, references);
            }
            syn::Expr::Block(expr_block) => {
                PublicIdentifier::visit_block(&expr_block.block, current_module_path, references);
            }
            syn::Expr::Assign(expr_assign) => {
                PublicIdentifier::visit_expr(&expr_assign.left, current_module_path, references);
                PublicIdentifier::visit_expr(&expr_assign.right, current_module_path, references);
            }

            syn::Expr::Return(expr_return) => {
                if let Some(expr) = &expr_return.expr {
                    PublicIdentifier::visit_expr(expr, current_module_path, references);
                }
            }
            syn::Expr::Break(expr_break) => {
                if let Some(expr) = &expr_break.expr {
                    PublicIdentifier::visit_expr(expr, current_module_path, references);
                }
            }
            syn::Expr::Continue(_) => {}
            syn::Expr::Tuple(expr_tuple) => {
                for elem in &expr_tuple.elems {
                    PublicIdentifier::visit_expr(elem, current_module_path, references);
                }
            }
            syn::Expr::Array(expr_array) => {
                for elem in &expr_array.elems {
                    PublicIdentifier::visit_expr(elem, current_module_path, references);
                }
            }
            syn::Expr::Repeat(expr_repeat) => {
                PublicIdentifier::visit_expr(&expr_repeat.expr, current_module_path, references);
                PublicIdentifier::visit_expr(&expr_repeat.len, current_module_path, references);
            }
            syn::Expr::Paren(expr_paren) => {
                PublicIdentifier::visit_expr(&expr_paren.expr, current_module_path, references);
            }
            syn::Expr::Group(expr_group) => {
                PublicIdentifier::visit_expr(&expr_group.expr, current_module_path, references);
            }
            syn::Expr::Try(expr_try) => {
                PublicIdentifier::visit_expr(&expr_try.expr, current_module_path, references);
            }
            syn::Expr::Async(expr_async) => {
                PublicIdentifier::visit_block(&expr_async.block, current_module_path, references);
            }
            syn::Expr::Await(expr_await) => {
                PublicIdentifier::visit_expr(&expr_await.base, current_module_path, references);
            }
            syn::Expr::TryBlock(expr_try_block) => {
                PublicIdentifier::visit_block(
                    &expr_try_block.block,
                    current_module_path,
                    references,
                );
            }
            syn::Expr::Yield(expr_yield) => {
                if let Some(expr) = &expr_yield.expr {
                    PublicIdentifier::visit_expr(expr, current_module_path, references);
                }
            }
            syn::Expr::Struct(expr_struct) => {
                PublicIdentifier::visit_path(&expr_struct.path, current_module_path, references);
                for field in &expr_struct.fields {
                    PublicIdentifier::visit_expr(&field.expr, current_module_path, references);
                }
                if let Some(rest) = &expr_struct.rest {
                    PublicIdentifier::visit_expr(rest, current_module_path, references);
                }
            }
            syn::Expr::Range(expr_range) => {
                if let Some(start) = &expr_range.start {
                    PublicIdentifier::visit_expr(start, current_module_path, references);
                }
                if let Some(end) = &expr_range.end {
                    PublicIdentifier::visit_expr(end, current_module_path, references);
                }
            }
            syn::Expr::Reference(expr_ref) => {
                PublicIdentifier::visit_expr(&expr_ref.expr, current_module_path, references);
            }
            syn::Expr::Unsafe(expr_unsafe) => {
                PublicIdentifier::visit_block(&expr_unsafe.block, current_module_path, references);
            }
            syn::Expr::Const(expr_const) => {
                PublicIdentifier::visit_block(&expr_const.block, current_module_path, references);
            }
            // Literal expressions don't contain path references
            syn::Expr::Lit(_) => {}
            syn::Expr::Macro(_) => {
                // Macro analysis is complex, skip for now
            }
            syn::Expr::Verbatim(_) => {}
            _ => {
                // Handle any new expression types that might be added
            }
        }
    }

    /// Visit a pattern to find type annotations
    fn visit_pat(
        pat: &syn::Pat,
        current_module_path: &str,
        references: &mut Vec<(String, String)>,
    ) {
        match pat {
            syn::Pat::Type(pat_type) => {
                PublicIdentifier::visit_type(&pat_type.ty, current_module_path, references);
                PublicIdentifier::visit_pat(&pat_type.pat, current_module_path, references);
            }
            syn::Pat::Tuple(pat_tuple) => {
                for elem in &pat_tuple.elems {
                    PublicIdentifier::visit_pat(elem, current_module_path, references);
                }
            }
            syn::Pat::Struct(pat_struct) => {
                PublicIdentifier::visit_path(&pat_struct.path, current_module_path, references);
                for field in &pat_struct.fields {
                    PublicIdentifier::visit_pat(&field.pat, current_module_path, references);
                }
            }
            syn::Pat::TupleStruct(pat_tuple_struct) => {
                PublicIdentifier::visit_path(
                    &pat_tuple_struct.path,
                    current_module_path,
                    references,
                );
                for elem in &pat_tuple_struct.elems {
                    PublicIdentifier::visit_pat(elem, current_module_path, references);
                }
            }
            syn::Pat::Reference(pat_ref) => {
                PublicIdentifier::visit_pat(&pat_ref.pat, current_module_path, references);
            }
            syn::Pat::Slice(pat_slice) => {
                for elem in &pat_slice.elems {
                    PublicIdentifier::visit_pat(elem, current_module_path, references);
                }
            }
            syn::Pat::Or(pat_or) => {
                for case in &pat_or.cases {
                    PublicIdentifier::visit_pat(case, current_module_path, references);
                }
            }
            _ => {
                // Other patterns (Ident, Lit, Wild, etc.) don't contain types
            }
        }
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
    pub dependencies: Vec<ModuleDependency>,
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
        let dependencies =
            PublicIdentifier::extract_dependencies_from_items(&parsed_file.items, name);

        Self {
            name: name.into(),
            usepath: name.into(),
            path: Some(path.into()),
            interface,
            children,
            dependencies,
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
                dependencies: vec![],
            };
        }

        let (children, interface, dependencies): (
            Vec<Module>,
            Vec<PublicIdentifier>,
            Vec<ModuleDependency>,
        ) = paths
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
                let dependencies =
                    PublicIdentifier::extract_dependencies_from_items(&parsed_file.items, usepath);

                (modules, interface, dependencies)
            })
            .fold((vec![], vec![], vec![]), |mut acc, x| {
                acc.0.extend(x.0);
                acc.1.extend(x.1);
                acc.2.extend(x.2);
                acc
            });

        Self {
            name: name.into(),
            usepath: usepath.into(),
            path: Some(path.into()),
            interface,
            children,
            dependencies,
        }
    }
}
