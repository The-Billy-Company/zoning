// Copyright (c) 2026 The Billy Company
//! Resolve written Rust paths through an indexed module tree, then retain their source spans.

use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use proc_macro2::{Span, TokenStream, TokenTree};
use syn::parse::Parser as _;
use syn::punctuated::Punctuated;
use syn::spanned::Spanned as _;
use syn::visit::{self, Visit};
use syn::{Item, ItemUse, Meta, Token, UseTree};

use super::cargo::{Cargo, normalize, posix};
use crate::Result;
use crate::survey::{Context, Destination, Reference};

type ModuleId = usize;

#[derive(Clone)]
struct Binding {
    name: Option<String>,
    path: Vec<String>,
    absolute: bool,
    offset: usize,
    module: Option<ModuleId>,
}

struct Module {
    source: usize,
    namespace: ModuleId,
    root: ModuleId,
    parent: Option<ModuleId>,
    directory: PathBuf,
    attribute_directory: PathBuf,
    items: Vec<Item>,
    children: BTreeMap<String, Vec<ModuleId>>,
    unjudged: BTreeMap<String, Vec<String>>,
    bindings: Vec<Binding>,
    definitions: BTreeSet<String>,
    orphan: bool,
}

struct Location {
    root: ModuleId,
    parent: Option<ModuleId>,
    directory: PathBuf,
    attribute_directory: PathBuf,
    depth: usize,
}

struct Graph<'a> {
    context: &'a Context<'a>,
    cargo: Cargo,
    parsed: Vec<syn::File>,
    by_path: BTreeMap<String, usize>,
    modules: Vec<Module>,
    library: Option<ModuleId>,
    effects: Vec<Reference>,
    owned: BTreeSet<usize>,
    fault: RefCell<Option<String>>,
    block_modules: BTreeMap<(ModuleId, usize), Vec<ModuleId>>,
}

pub(super) fn references(context: &Context<'_>) -> Result<Vec<Reference>> {
    let cargo = Cargo::read(context)?;
    let parsed = context
        .sources
        .iter()
        .map(|source| {
            syn::parse_file(&source.text).map_err(|error| {
                let start = error.span().start();
                format!(
                    "{}:{}:{}: cannot parse Rust source: {error}",
                    context.module_root.join(&source.path).display(),
                    start.line,
                    start.column + 1
                )
                .into()
            })
        })
        .collect::<Result<Vec<_>>>()?;
    let mut graph = Graph {
        context,
        cargo,
        parsed,
        by_path: context
            .sources
            .iter()
            .enumerate()
            .map(|(index, source)| (source.path.clone(), index))
            .collect(),
        modules: Vec::new(),
        library: None,
        effects: Vec::new(),
        owned: BTreeSet::new(),
        fault: RefCell::new(None),
        block_modules: BTreeMap::new(),
    };
    let roots = graph.cargo.roots.clone();
    for root in roots {
        if let Some(&source) = graph.by_path.get(&root) {
            let id = graph.add_root(source)?;
            if graph.cargo.library.as_ref().is_some_and(|(_, library)| *library == root) {
                graph.library = Some(id);
            }
        }
    }
    // A source-only gate also judges examples and research files which Cargo does
    // not compile. Their crate anchor is the library, but no fabricated declaration
    // edge makes them dependencies of production code.
    for source in 0..context.sources.len() {
        if !graph.owned.contains(&source) {
            let anchor = graph.library;
            let id = graph.add_root(source)?;
            graph.modules[id].orphan = true;
            if let Some(anchor) = anchor {
                graph.modules[id].root = anchor;
            }
        }
    }
    let mut references = std::mem::take(&mut graph.effects);
    for module in 0..graph.modules.len() {
        let mut visitor = Reader {
            graph: &graph,
            module,
            locals: Vec::new(),
            references: Vec::new(),
            fault: None,
        };
        for item in &graph.modules[module].items {
            visitor.visit_item(item);
        }
        if let Some(error) = visitor.fault {
            return Err(error.into());
        }
        if let Some(error) = graph.fault.take() {
            return Err(error.into());
        }
        references.extend(visitor.references);
    }
    references.sort_by(|one, two| {
        (&one.source, one.offset, &one.spec).cmp(&(&two.source, two.offset, &two.spec))
    });
    references.dedup_by(|one, two| {
        one.source == two.source
            && one.offset == two.offset
            && one.spec == two.spec
            && same_destination(&one.destination, &two.destination)
    });
    Ok(references)
}

fn same_destination(one: &Destination, two: &Destination) -> bool {
    match (one, two) {
        (
            Destination::Local { path: one, hops: one_hops },
            Destination::Local { path: two, hops: two_hops },
        ) => one == two && one_hops == two_hops,
        (Destination::Outside(one), Destination::Outside(two)) => one == two,
        (Destination::Escape, Destination::Escape) => true,
        _ => false,
    }
}

impl Graph<'_> {
    fn add_root(&mut self, source: usize) -> Result<ModuleId> {
        let directory = Path::new(&self.context.sources[source].path)
            .parent()
            .unwrap_or(Path::new(""))
            .to_path_buf();
        let location = Location {
            root: self.modules.len(),
            parent: None,
            directory: directory.clone(),
            attribute_directory: directory,
            depth: 0,
        };
        self.add_module(source, location, self.parsed[source].items.clone())
    }

    fn add_module(
        &mut self,
        source: usize,
        location: Location,
        items: Vec<Item>,
    ) -> Result<ModuleId> {
        if location.depth > 128
            || self.modules.len() >= self.context.sources.len().saturating_mul(256).max(256)
        {
            return Err("Rust module tree exceeds the bounded survey size; check recursive #[path] declarations".into());
        }
        let id = self.modules.len();
        let text = &self.context.sources[source].text;
        let bindings = items.iter().flat_map(|item| item_bindings(item, text)).collect();
        let mut definitions: BTreeSet<String> = items.iter().filter_map(definition).collect();
        for item in &items {
            if let Item::Macro(item) = item {
                static_definitions(item.mac.tokens.clone(), &mut definitions);
            }
        }
        let mut ancestor = location.parent;
        while let Some(above) = ancestor {
            if self.modules[above].source == source && self.modules[above].items == items {
                return Err(format!(
                    "{}: recursive Rust module ownership through #[path]",
                    self.context.sources[source].path
                )
                .into());
            }
            ancestor = self.modules[above].parent;
        }
        self.modules.push(Module {
            source,
            namespace: id,
            root: location.root,
            parent: location.parent,
            directory: location.directory,
            attribute_directory: location.attribute_directory,
            items: items.clone(),
            children: BTreeMap::new(),
            unjudged: BTreeMap::new(),
            bindings,
            definitions,
            orphan: false,
        });
        self.owned.insert(source);
        for item in items {
            if let Item::Macro(include) = &item {
                self.add_include(id, &include.mac, location.depth)?;
            }
            let declarations = match item {
                Item::Mod(module) => vec![(module, false)],
                item => {
                    let mut collector = LocalModules(Vec::new());
                    collector.visit_item(&item);
                    collector.0.into_iter().map(|module| (module, true)).collect()
                }
            };
            for (module, local) in declarations {
                self.add_declaration(id, &module, local, location.depth)?;
            }
        }
        Ok(id)
    }

    fn add_include(&mut self, id: ModuleId, include: &syn::Macro, depth: usize) -> Result<()> {
        if !include.path.is_ident("include") {
            return Ok(());
        }
        let Ok(path) = syn::parse2::<syn::LitStr>(include.tokens.clone()) else {
            return Ok(());
        };
        let source = self.modules[id].source;
        let file = Path::new(&self.context.sources[source].path)
            .parent()
            .unwrap_or(Path::new(""))
            .join(path.value());
        let absolute = normalize(&self.context.module_root.join(file));
        let root = normalize(self.context.module_root);
        let Ok(path) = absolute.strip_prefix(root) else {
            return Ok(());
        };
        let Some(&included) = self.by_path.get(&posix(path)) else {
            return Ok(());
        };
        let directory = Path::new(&self.context.sources[included].path)
            .parent()
            .unwrap_or(Path::new(""))
            .to_path_buf();
        let location = Location {
            root: self.modules[id].root,
            parent: self.modules[id].parent,
            directory: directory.clone(),
            attribute_directory: directory,
            depth: depth + 1,
        };
        let child = self.add_module(included, location, self.parsed[included].items.clone())?;
        self.modules[child].namespace = id;
        let children = self.modules[child].children.clone();
        let unjudged = self.modules[child].unjudged.clone();
        let bindings = self.modules[child].bindings.clone();
        let definitions = self.modules[child].definitions.clone();
        self.modules[id].children.extend(children);
        self.modules[id].unjudged.extend(unjudged);
        self.modules[id].bindings.extend(bindings);
        self.modules[id].definitions.extend(definitions);
        Ok(())
    }

    fn add_declaration(
        &mut self,
        parent: ModuleId,
        module: &syn::ItemMod,
        local: bool,
        depth: usize,
    ) -> Result<()> {
        if let Some((_, items)) = &module.content {
            let directory = self.modules[parent].directory.join(identifier(&module.ident));
            let location = Location {
                root: self.modules[parent].root,
                parent: Some(parent),
                directory: directory.clone(),
                attribute_directory: directory,
                depth: depth + 1,
            };
            let child = self.add_module(self.modules[parent].source, location, items.clone())?;
            self.attach(parent, module, local, child);
            return Ok(());
        }
        let mut found = BTreeSet::new();
        let mut present = false;
        for path in self.module_candidates(parent, module)? {
            if found.insert(path.clone()) {
                present |= self.load_module(parent, module, local, &path, depth)?;
            }
        }
        if !present {
            let source = &self.context.sources[self.modules[parent].source];
            return Err(format!("{}:{}: Rust module `{}` has no source file; add the declared file or correct #[path]", source.path, module.span().start().line, module.ident).into());
        }
        Ok(())
    }

    fn module_candidates(&self, parent: ModuleId, module: &syn::ItemMod) -> Result<Vec<PathBuf>> {
        let paths = module_paths(&module.attrs)?;
        let mut candidates = Vec::new();
        if paths.default {
            let base = self.modules[parent].directory.join(identifier(&module.ident));
            if self.context.module_root.join(base.with_extension("rs")).is_file()
                && self.context.module_root.join(base.join("mod.rs")).is_file()
            {
                let source = &self.context.sources[self.modules[parent].source];
                return Err(format!("{}:{}: Rust module `{}` is ambiguous: both {} and {} exist; keep one source location", source.path, module.span().start().line, module.ident, base.with_extension("rs").display(), base.join("mod.rs").display()).into());
            }
            candidates.push(base.with_extension("rs"));
            candidates.push(base.join("mod.rs"));
        }
        candidates.extend(
            paths
                .explicit
                .into_iter()
                .map(|path| self.modules[parent].attribute_directory.join(path)),
        );
        Ok(candidates)
    }

    fn load_module(
        &mut self,
        parent: ModuleId,
        module: &syn::ItemMod,
        local: bool,
        path: &Path,
        depth: usize,
    ) -> Result<bool> {
        let absolute = normalize(&self.context.module_root.join(path));
        let module_root = normalize(self.context.module_root);
        let name = identifier(&module.ident);
        let Ok(relative) = absolute.strip_prefix(&module_root) else {
            let source = &self.context.sources[self.modules[parent].source];
            self.effects.push(Reference {
                source: source.path.clone(),
                offset: offset(&source.text, module.span()),
                spec: posix(path),
                destination: Destination::Escape,
            });
            self.modules[parent].unjudged.entry(name).or_default().push(posix(path));
            return Ok(true);
        };
        let relative = posix(relative);
        let Some(&source) = self.by_path.get(&relative) else {
            if absolute.is_file() {
                self.modules[parent].unjudged.entry(name).or_default().push(relative);
                return Ok(true);
            }
            return Ok(false);
        };
        let containing = Path::new(&relative).parent().unwrap_or(Path::new(""));
        let directory = if relative.ends_with("/mod.rs") || relative == "mod.rs" {
            containing.to_path_buf()
        } else {
            containing.join(Path::new(&relative).file_stem().unwrap_or_default())
        };
        let location = Location {
            root: self.modules[parent].root,
            parent: Some(parent),
            directory,
            attribute_directory: containing.to_path_buf(),
            depth: depth + 1,
        };
        let child = self.add_module(source, location, self.parsed[source].items.clone())?;
        self.attach(parent, module, local, child);
        Ok(true)
    }

    fn attach(&mut self, parent: ModuleId, module: &syn::ItemMod, local: bool, child: ModuleId) {
        if local {
            let text = &self.context.sources[self.modules[parent].source].text;
            self.block_modules
                .entry((parent, offset(text, module.span())))
                .or_default()
                .push(child);
        } else {
            self.modules[parent].children.entry(identifier(&module.ident)).or_default().push(child);
        }
    }

    fn resolve(
        &self,
        from: ModuleId,
        path: &[String],
        absolute: bool,
        locals: &[Vec<Binding>],
        depth: usize,
    ) -> Vec<Destination> {
        let Some(head) = path.first() else {
            return Vec::new();
        };
        if depth > 64 {
            *self.fault.borrow_mut() = Some(format!(
                "Rust name resolution exceeds 64 alias/module steps at `{}`; remove the recursive alias chain",
                path.join("::")
            ));
            return Vec::new();
        }
        let from = self.modules[from].namespace;
        if head == "Self" {
            return Vec::new();
        }
        if self.modules[from].orphan && matches!(head.as_str(), "crate" | "self" | "super") {
            *self.fault.borrow_mut() = Some(format!(
                "{}: cannot resolve `{}` without Rust module ownership; declare source modules explicitly instead of generating mod declarations",
                self.context.sources[self.modules[from].source].path,
                path.join("::")
            ));
            return Vec::new();
        }
        if absolute && !matches!(head.as_str(), "crate" | "self" | "super") {
            return self
                .library_path(from, path, depth)
                .unwrap_or_else(|| vec![Destination::Outside(head.clone())]);
        }
        let mut module = from;
        let mut at = 0;
        let mut hops = 0;
        match head.as_str() {
            "crate" => {
                module = self.modules[from].root;
                at = 1;
            }
            "self" => {
                at = 1;
            }
            "super" => {
                while path.get(at).is_some_and(|part| part == "super") {
                    let Some(parent) = self.modules[module].parent else {
                        return vec![Destination::Escape];
                    };
                    module = parent;
                    at += 1;
                    hops += 1;
                }
            }
            _ => return self.named_path(from, path, locals, depth),
        }
        self.walk(module, &path[at..], hops, true, depth)
    }

    fn library_path(
        &self,
        from: ModuleId,
        path: &[String],
        depth: usize,
    ) -> Option<Vec<Destination>> {
        let library = self.library?;
        let (name, _) = self.cargo.library.as_ref()?;
        if path.first()? != name {
            return None;
        }
        if self.modules[from].root != library {
            return Some(vec![Destination::Outside(name.clone())]);
        }
        Some(self.walk(library, &path[1..], 0, true, depth + 1))
    }

    fn named_path(
        &self,
        from: ModuleId,
        path: &[String],
        locals: &[Vec<Binding>],
        depth: usize,
    ) -> Vec<Destination> {
        let head = &path[0];
        let scope = &self.modules[from];
        if !scope.children.contains_key(head)
            && !scope.definitions.contains(head)
            && let Some(destination) = self.library_path(from, path, depth)
        {
            return destination;
        }
        for bindings in locals.iter().rev() {
            if let Some(binding) =
                bindings.iter().rev().find(|binding| binding.name.as_ref() == Some(head))
            {
                if let Some(module) = binding.module {
                    return self.walk(module, &path[1..], 0, true, depth + 1);
                }
                if let Some(destinations) = self.bound_path(from, binding, path, depth) {
                    return destinations;
                }
            }
        }
        if let Some(binding) = scope.bindings.iter().find(|binding| {
            binding.name.as_ref() == Some(head) && !scope.children.contains_key(head)
        }) && let Some(destinations) = self.bound_path(from, binding, path, depth)
        {
            return destinations;
        }
        if !scope.children.contains_key(head)
            && !scope.definitions.contains(head)
            && (self.cargo.externs.contains(head) || RustAmbient::contains(head))
        {
            return vec![Destination::Outside(head.clone())];
        }
        self.walk(from, path, 0, false, depth)
    }

    fn bound_path(
        &self,
        from: ModuleId,
        binding: &Binding,
        path: &[String],
        depth: usize,
    ) -> Option<Vec<Destination>> {
        let namespace = self.namespace(from, &binding.path, binding.absolute, depth + 1);
        let mut expanded = binding.path.clone();
        if !namespace.is_empty() {
            expanded.extend_from_slice(&path[1..]);
        }
        (expanded != path).then(|| {
            let entrance = self.resolve(from, &binding.path, binding.absolute, &[], depth + 1);
            self.resolve(from, &expanded, binding.absolute, &[], depth + 1)
                .into_iter()
                .flat_map(|destination| {
                    if self.in_namespace_file(&destination, &namespace) {
                        entrance.clone()
                    } else {
                        vec![destination]
                    }
                })
                .collect()
        })
    }

    fn walk(
        &self,
        module: ModuleId,
        path: &[String],
        hops: u32,
        anchored: bool,
        depth: usize,
    ) -> Vec<Destination> {
        if depth > 64 {
            *self.fault.borrow_mut() = Some(format!(
                "Rust name resolution exceeds 64 alias/module steps at `{}`",
                path.join("::")
            ));
            return Vec::new();
        }
        if path.is_empty() {
            return vec![self.local(module, hops)];
        }
        let head = &path[0];
        if let Some(children) = self.modules[module].children.get(head) {
            return children
                .iter()
                .flat_map(|child| self.walk(*child, &path[1..], hops, true, depth + 1))
                .collect();
        }
        if let Some(paths) = self.modules[module].unjudged.get(head) {
            return paths
                .iter()
                .map(|path| Destination::Local { path: path.clone(), hops })
                .collect();
        }
        if (self.modules[module].root == module || path.len() > 1)
            && let Some(binding) = self.modules[module]
                .bindings
                .iter()
                .find(|binding| binding.name.as_ref() == Some(head))
        {
            let namespace = self.namespace(module, &binding.path, binding.absolute, depth + 1);
            if namespace.is_empty() && self.modules[module].root != module {
                return vec![self.local(module, hops)];
            }
            let mut expanded = binding.path.clone();
            if !namespace.is_empty() {
                expanded.extend_from_slice(&path[1..]);
            }
            let resolved = self.resolve(module, &expanded, binding.absolute, &[], depth + 1);
            if !resolved.is_empty() {
                return resolved
                    .into_iter()
                    .map(|destination| {
                        if self.modules[module].root != module
                            && self.in_namespace_file(&destination, &namespace)
                        {
                            self.local(module, hops)
                        } else {
                            destination
                        }
                    })
                    .collect();
            }
        }
        if self.modules[module].definitions.contains(head)
            || self.modules[module]
                .bindings
                .iter()
                .any(|binding| binding.name.as_ref() == Some(head))
            || (anchored && (path.len() == 1 || self.modules[module].root != module))
        {
            return vec![self.local(module, hops)];
        }
        self.modules[module]
            .bindings
            .iter()
            .filter(|binding| {
                binding.name.is_none()
                    && !self
                        .namespace(module, &binding.path, binding.absolute, depth + 1)
                        .is_empty()
            })
            .flat_map(|binding| {
                let mut expanded = binding.path.clone();
                expanded.extend_from_slice(path);
                self.resolve(module, &expanded, binding.absolute, &[], depth + 1)
            })
            .collect()
    }

    fn local(&self, module: ModuleId, hops: u32) -> Destination {
        Destination::Local {
            path: self.context.sources[self.modules[module].source].path.clone(),
            hops,
        }
    }

    fn in_namespace_file(&self, destination: &Destination, namespace: &[ModuleId]) -> bool {
        let Destination::Local { path, .. } = destination else {
            return false;
        };
        namespace
            .iter()
            .any(|module| self.context.sources[self.modules[*module].source].path == *path)
    }

    fn namespace(
        &self,
        from: ModuleId,
        path: &[String],
        absolute: bool,
        depth: usize,
    ) -> Vec<ModuleId> {
        if path.is_empty() || depth > 64 {
            return Vec::new();
        }
        let from = self.modules[from].namespace;
        let mut module = from;
        let mut at = 0;
        match path[0].as_str() {
            "crate" => {
                module = self.modules[from].root;
                at = 1;
            }
            "self" => {
                at = 1;
            }
            "super" => {
                while path.get(at).is_some_and(|part| part == "super") {
                    let Some(parent) = self.modules[module].parent else { return Vec::new() };
                    module = parent;
                    at += 1;
                }
            }
            _ if absolute => return Vec::new(),
            _ => {}
        }
        self.namespace_walk(module, &path[at..], depth + 1)
    }

    fn namespace_walk(&self, module: ModuleId, path: &[String], depth: usize) -> Vec<ModuleId> {
        if depth > 64 {
            return Vec::new();
        }
        let Some(head) = path.first() else { return vec![module] };
        if let Some(children) = self.modules[module].children.get(head) {
            return children
                .iter()
                .flat_map(|child| self.namespace_walk(*child, &path[1..], depth + 1))
                .collect();
        }
        if let Some(binding) =
            self.modules[module].bindings.iter().find(|binding| binding.name.as_ref() == Some(head))
        {
            let mut expanded = binding.path.clone();
            expanded.extend_from_slice(&path[1..]);
            if expanded != path {
                return self.namespace(module, &expanded, binding.absolute, depth + 1);
            }
        }
        Vec::new()
    }
}

fn static_definitions(tokens: TokenStream, definitions: &mut BTreeSet<String>) {
    let tokens: Vec<TokenTree> = tokens.into_iter().collect();
    for (at, token) in tokens.iter().enumerate() {
        match token {
            TokenTree::Group(group) => static_definitions(group.stream(), definitions),
            TokenTree::Ident(keyword)
                if matches!(
                    keyword.to_string().as_str(),
                    "enum"
                        | "struct"
                        | "union"
                        | "trait"
                        | "type"
                        | "const"
                        | "static"
                        | "fn"
                        | "mod"
                ) =>
            {
                if let Some(TokenTree::Ident(name)) = tokens.get(at + 1) {
                    definitions.insert(identifier(name));
                }
            }
            _ => {}
        }
    }
}

struct RustAmbient;
impl RustAmbient {
    fn contains(name: &str) -> bool {
        matches!(name, "std" | "core" | "alloc" | "proc_macro" | "test")
    }
}

struct LocalModules(Vec<syn::ItemMod>);
impl<'ast> Visit<'ast> for LocalModules {
    fn visit_item_mod(&mut self, module: &'ast syn::ItemMod) {
        self.0.push(module.clone());
    }
}

fn definition(item: &Item) -> Option<String> {
    let ident = match item {
        Item::Const(item) => &item.ident,
        Item::Enum(item) => &item.ident,
        Item::Fn(item) => &item.sig.ident,
        Item::Static(item) => &item.ident,
        Item::Struct(item) => &item.ident,
        Item::Trait(item) => &item.ident,
        Item::TraitAlias(item) => &item.ident,
        Item::Type(item) => &item.ident,
        Item::Union(item) => &item.ident,
        Item::Macro(item) => return item.ident.as_ref().map(identifier),
        _ => return None,
    };
    Some(identifier(ident))
}

fn bindings(item: &ItemUse, text: &str) -> Vec<Binding> {
    fn flatten(
        tree: &UseTree,
        prefix: &mut Vec<String>,
        absolute: bool,
        offset: usize,
        out: &mut Vec<Binding>,
    ) {
        match tree {
            UseTree::Path(path) => {
                prefix.push(identifier(&path.ident));
                flatten(&path.tree, prefix, absolute, offset, out);
                prefix.pop();
            }
            UseTree::Name(name) => {
                let name = identifier(&name.ident);
                if name == "self" {
                    out.push(Binding {
                        name: prefix.last().cloned(),
                        path: prefix.clone(),
                        absolute,
                        offset,
                        module: None,
                    });
                } else {
                    let mut path = prefix.clone();
                    path.push(name.clone());
                    out.push(Binding { name: Some(name), path, absolute, offset, module: None });
                }
            }
            UseTree::Rename(rename) => {
                let mut path = prefix.clone();
                if rename.ident != "self" {
                    path.push(identifier(&rename.ident));
                }
                out.push(Binding {
                    name: Some(identifier(&rename.rename)),
                    path,
                    absolute,
                    offset,
                    module: None,
                });
            }
            UseTree::Glob(_) => out.push(Binding {
                name: None,
                path: prefix.clone(),
                absolute,
                offset,
                module: None,
            }),
            UseTree::Group(group) => {
                for tree in &group.items {
                    flatten(tree, prefix, absolute, offset, out);
                }
            }
        }
    }
    let mut out = Vec::new();
    flatten(
        &item.tree,
        &mut Vec::new(),
        item.leading_colon.is_some(),
        offset(text, item.span()),
        &mut out,
    );
    out
}

fn item_bindings(item: &Item, text: &str) -> Vec<Binding> {
    match item {
        Item::Use(item) => bindings(item, text),
        Item::ExternCrate(item) => vec![Binding {
            name: Some(
                item.rename
                    .as_ref()
                    .map_or_else(|| identifier(&item.ident), |(_, name)| identifier(name)),
            ),
            path: vec![if item.ident == "self" {
                "crate".to_owned()
            } else {
                identifier(&item.ident)
            }],
            absolute: item.ident != "self",
            offset: offset(text, item.span()),
            module: None,
        }],
        _ => Vec::new(),
    }
}

struct ModulePaths {
    explicit: Vec<String>,
    default: bool,
}

fn module_paths(attributes: &[syn::Attribute]) -> Result<ModulePaths> {
    fn paths(meta: &Meta, found: &mut Vec<String>) -> Result<()> {
        match meta {
            Meta::NameValue(value) if value.path.is_ident("path") => {
                if let syn::Expr::Lit(value) = &value.value
                    && let syn::Lit::Str(path) = &value.lit
                {
                    found.push(path.value());
                    return Ok(());
                }
                return Err("Rust #[path] must name a literal source path".into());
            }
            Meta::List(list) if list.path.is_ident("cfg_attr") => {
                let nested = Punctuated::<Meta, Token![,]>::parse_terminated
                    .parse2(list.tokens.clone())
                    .map_err(|error| format!("cannot parse Rust cfg_attr: {error}"))?;
                for meta in nested.iter().skip(1) {
                    paths(meta, found)?;
                }
            }
            _ => {}
        }
        Ok(())
    }
    let mut explicit = Vec::new();
    let mut default = true;
    for attribute in attributes {
        if attribute.path().is_ident("path") {
            default = false;
        }
        paths(&attribute.meta, &mut explicit)?;
    }
    Ok(ModulePaths { explicit, default })
}

struct Reader<'a, 'b> {
    graph: &'a Graph<'b>,
    module: ModuleId,
    locals: Vec<Vec<Binding>>,
    references: Vec<Reference>,
    fault: Option<String>,
}

impl Reader<'_, '_> {
    fn emit(&mut self, path: &[String], absolute: bool, span: Span) {
        let destinations = self.graph.resolve(self.module, path, absolute, &self.locals, 0);
        let source = &self.graph.context.sources[self.graph.modules[self.module].source];
        if destinations.is_empty()
            && path.first().is_some_and(|head| matches!(head.as_str(), "crate" | "self" | "super"))
        {
            self.fault = Some(format!(
                "{}:{}: cannot resolve Rust path `{}`",
                source.path,
                span.start().line,
                path.join("::")
            ));
        }
        for destination in destinations {
            self.references.push(Reference {
                source: source.path.clone(),
                offset: offset(&source.text, span),
                spec: format!("{}{}", if absolute { "::" } else { "" }, path.join("::")),
                destination,
            });
        }
    }

    fn scan_tokens(&mut self, tokens: TokenStream, prefix: &[String], absolute: bool) {
        let tokens: Vec<TokenTree> = tokens.into_iter().collect();
        let mut at = 0;
        while at < tokens.len() {
            if let TokenTree::Group(group) = &tokens[at] {
                self.scan_tokens(group.stream(), &[], false);
                at += 1;
                continue;
            }
            let TokenTree::Ident(first) = &tokens[at] else {
                at += 1;
                continue;
            };
            let leading = prefix.is_empty() && absolute_prefix(&tokens, at);
            let rooted = absolute || leading;
            let mut path = prefix.to_vec();
            path.push(identifier(first));
            let mut end = at + 1;
            while end + 2 < tokens.len() && colon(&tokens[end]) && colon(&tokens[end + 1]) {
                if let TokenTree::Ident(next) = &tokens[end + 2] {
                    path.push(identifier(next));
                    end += 3;
                } else {
                    break;
                }
            }
            if path.len() > prefix.len() + 1 || !prefix.is_empty() {
                let span = if leading { tokens[at - 2].span() } else { first.span() };
                self.emit(&path, rooted, span);
            }
            if end + 2 < tokens.len()
                && colon(&tokens[end])
                && colon(&tokens[end + 1])
                && let TokenTree::Group(group) = &tokens[end + 2]
            {
                self.scan_tokens(group.stream(), &path, rooted);
                end += 3;
            }
            at = end;
        }
    }

    fn include(&mut self, item: &syn::Macro) {
        let source = &self.graph.context.sources[self.graph.modules[self.module].source];
        let Ok(path) = syn::parse2::<syn::LitStr>(item.tokens.clone()) else {
            self.fault = Some(format!(
                "{}:{}: include! requires a literal path for a complete source survey; generated Rust is not expanded",
                source.path,
                item.span().start().line
            ));
            return;
        };
        let parent = Path::new(&source.path).parent().unwrap_or(Path::new(""));
        let written = path.value();
        let hops = Path::new(&written)
            .components()
            .filter(|component| matches!(component, std::path::Component::ParentDir))
            .count() as u32;
        let absolute = normalize(&self.graph.context.module_root.join(parent).join(&written));
        let root = normalize(self.graph.context.module_root);
        if let Ok(relative) = absolute.strip_prefix(&root) {
            if relative.extension().is_none_or(|extension| extension != "rs") {
                self.fault = Some(format!(
                    "{}:{}: include! source `{written}` is not governed Rust source; this survey requires a .rs included file instead of arbitrary source extensions",
                    source.path,
                    item.span().start().line
                ));
                return;
            }
            if !self.graph.by_path.contains_key(&posix(relative)) && !absolute.is_file() {
                self.fault = Some(format!(
                    "{}:{}: include! source `{written}` is missing; add the included .rs file or correct its literal path",
                    source.path,
                    item.span().start().line
                ));
                return;
            }
        }
        let destination = absolute
            .strip_prefix(&root)
            .map_or(Destination::Escape, |path| Destination::Local { path: posix(path), hops });
        self.references.push(Reference {
            source: source.path.clone(),
            offset: offset(&source.text, item.span()),
            spec: written,
            destination,
        });
    }
}

impl<'ast> Visit<'ast> for Reader<'_, '_> {
    fn visit_item_mod(&mut self, _item: &'ast syn::ItemMod) {}

    fn visit_attribute(&mut self, attribute: &'ast syn::Attribute) {
        self.visit_path(attribute.path());
        if let Meta::List(list) = &attribute.meta {
            self.scan_tokens(list.tokens.clone(), &[], false);
        }
    }

    fn visit_item_use(&mut self, item: &'ast ItemUse) {
        let source = &self.graph.context.sources[self.graph.modules[self.module].source];
        for binding in bindings(item, &source.text) {
            let mut destinations =
                self.graph.resolve(self.module, &binding.path, binding.absolute, &self.locals, 0);
            if destinations.is_empty()
                && let Some(head) = binding.path.first()
            {
                if matches!(head.as_str(), "crate" | "self" | "super") {
                    self.fault = Some(format!(
                        "{}:{}: cannot resolve Rust import `{}`",
                        source.path,
                        item.span().start().line,
                        binding.path.join("::")
                    ));
                } else {
                    destinations.push(Destination::Outside(head.clone()));
                }
            }
            for destination in destinations {
                self.references.push(Reference {
                    source: source.path.clone(),
                    offset: binding.offset,
                    spec: binding.path.join("::"),
                    destination,
                });
            }
        }
    }

    fn visit_item_extern_crate(&mut self, item: &'ast syn::ItemExternCrate) {
        let path =
            vec![if item.ident == "self" { "crate".to_owned() } else { identifier(&item.ident) }];
        self.emit(&path, item.ident != "self", item.span());
    }

    fn visit_path(&mut self, path: &'ast syn::Path) {
        let parts: Vec<String> = path.segments.iter().map(|part| identifier(&part.ident)).collect();
        if parts.len() > 1 || path.leading_colon.is_some() {
            self.emit(&parts, path.leading_colon.is_some(), path.span());
        }
        visit::visit_path(self, path);
    }

    fn visit_expr_path(&mut self, expression: &'ast syn::ExprPath) {
        if let Some(qualified) = &expression.qself {
            self.visit_qself(qualified);
            if qualified.position > 0 {
                let parts: Vec<String> = expression
                    .path
                    .segments
                    .iter()
                    .take(qualified.position)
                    .map(|part| identifier(&part.ident))
                    .collect();
                self.emit(&parts, expression.path.leading_colon.is_some(), expression.path.span());
            }
            for segment in &expression.path.segments {
                self.visit_path_arguments(&segment.arguments);
            }
        } else {
            visit::visit_expr_path(self, expression);
        }
    }

    fn visit_type_path(&mut self, type_path: &'ast syn::TypePath) {
        if let Some(qualified) = &type_path.qself {
            self.visit_qself(qualified);
            if qualified.position > 0 {
                let parts: Vec<String> = type_path
                    .path
                    .segments
                    .iter()
                    .take(qualified.position)
                    .map(|part| identifier(&part.ident))
                    .collect();
                self.emit(&parts, type_path.path.leading_colon.is_some(), type_path.path.span());
            }
            for segment in &type_path.path.segments {
                self.visit_path_arguments(&segment.arguments);
            }
        } else {
            visit::visit_type_path(self, type_path);
        }
    }

    fn visit_block(&mut self, block: &'ast syn::Block) {
        let source = &self.graph.context.sources[self.graph.modules[self.module].source];
        let mut imports: Vec<Binding> = block
            .stmts
            .iter()
            .filter_map(
                |statement| if let syn::Stmt::Item(item) = statement { Some(item) } else { None },
            )
            .flat_map(|item| item_bindings(item, &source.text))
            .collect();
        for statement in &block.stmts {
            if let syn::Stmt::Item(Item::Mod(module)) = statement {
                let declaration_offset = offset(&source.text, module.span());
                if let Some(children) =
                    self.graph.block_modules.get(&(self.module, declaration_offset))
                {
                    imports.extend(children.iter().map(|child| Binding {
                        name: Some(identifier(&module.ident)),
                        path: Vec::new(),
                        absolute: false,
                        offset: declaration_offset,
                        module: Some(*child),
                    }));
                }
            }
        }
        self.locals.push(imports);
        visit::visit_block(self, block);
        self.locals.pop();
    }

    fn visit_macro(&mut self, item: &'ast syn::Macro) {
        self.visit_path(&item.path);
        if item.path.is_ident("include") {
            self.include(item);
        } else {
            self.scan_tokens(item.tokens.clone(), &[], false);
        }
    }
}

fn absolute_prefix(tokens: &[TokenTree], at: usize) -> bool {
    if at < 2 || !colon(&tokens[at - 2]) || !colon(&tokens[at - 1]) {
        return false;
    }
    // A qualified self type ends with `>` before `::`; arrows introduce a path.
    let qualified = at >= 3
        && matches!(&tokens[at - 3], TokenTree::Punct(punctuation) if punctuation.as_char() == '>')
        && !(at >= 4
            && matches!(&tokens[at - 4], TokenTree::Punct(punctuation) if matches!(punctuation.as_char(), '=' | '-')));
    !qualified
}

fn colon(token: &TokenTree) -> bool {
    matches!(token, TokenTree::Punct(punctuation) if punctuation.as_char() == ':')
}

fn offset(text: &str, span: Span) -> usize {
    let location = span.start();
    let line = text
        .split_inclusive('\n')
        .take(location.line.saturating_sub(1))
        .map(str::len)
        .sum::<usize>();
    let column = text[line.min(text.len())..]
        .chars()
        .take(location.column)
        .map(char::len_utf8)
        .sum::<usize>();
    (line + column).min(text.len())
}

fn identifier(ident: &proc_macro2::Ident) -> String {
    let name = ident.to_string();
    name.strip_prefix("r#").unwrap_or(&name).to_owned()
}
