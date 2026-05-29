use crate::platform;
use crate::{FieldRef, MethodRef};
use javac_ty::check::{boxing_type, is_assignable, unboxing_type};
use javac_ty::descriptor::{descriptor_to_ty, method_descriptor_to_sig};
use javac_ty::{MethodSig, Ty};
use std::cmp::Ordering;
use std::collections::{HashMap, HashSet, VecDeque};

const ACC_STATIC: u16 = 0x0008;
const ACC_VARARGS: u16 = 0x0080;
const OBJECT_CLASS: &str = "java/lang/Object";

#[derive(Debug, Clone, Default)]
pub struct ClassCatalog {
    classes: HashSet<String>,
    packages: HashSet<String>,
    simple_names: HashMap<String, SimpleName>,
    fields: HashMap<(String, String), FieldRef>,
    methods: HashMap<(String, String), Vec<MethodRef>>,
    interfaces: HashSet<String>,
    parents: HashMap<String, Vec<String>>,
}

#[derive(Debug, Clone)]
enum SimpleName {
    Unique(String),
    Ambiguous,
}

impl ClassCatalog {
    pub fn platform() -> Self {
        let mut catalog = Self::default();
        for class_name in platform::classes() {
            catalog.insert_internal_class(class_name);
        }
        for interface_name in platform::interfaces() {
            catalog.mark_interface(interface_name);
        }
        for parent in platform::parents() {
            catalog.insert_parent(parent.owner, parent.parent);
        }
        for field in platform::fields() {
            if let Some(ty) = descriptor_to_ty(field.descriptor) {
                catalog.insert_field(
                    field.owner,
                    field.name,
                    field.descriptor,
                    ty,
                    field.access_flags,
                );
            }
        }
        for method in platform::methods() {
            if let Some(sig) = method_descriptor_to_sig(method.name, method.descriptor) {
                catalog.insert_method(method.owner, sig, method.access_flags, method.is_interface);
            }
        }
        catalog
    }

    pub fn insert_internal_class(&mut self, internal_name: impl AsRef<str>) {
        let Some(internal_name) = normalize_internal_name(internal_name.as_ref()) else {
            return;
        };
        if !self.classes.insert(internal_name.clone()) {
            return;
        }

        if let Some((package, simple_name)) = internal_name.rsplit_once('/') {
            self.packages.insert(package.to_string());
            self.insert_simple_name(simple_name, &internal_name);
        } else {
            self.insert_simple_name(&internal_name, &internal_name);
        }
    }

    pub fn mark_interface(&mut self, internal_name: impl AsRef<str>) {
        if let Some(internal_name) = normalize_internal_name(internal_name.as_ref()) {
            self.interfaces.insert(internal_name);
        }
    }

    pub fn insert_parent(&mut self, owner: impl AsRef<str>, parent: impl AsRef<str>) {
        let Some(owner) = normalize_internal_name(owner.as_ref()) else {
            return;
        };
        let Some(parent) = normalize_internal_name(parent.as_ref()) else {
            return;
        };
        if owner == parent {
            return;
        }
        let parents = self.parents.entry(owner).or_default();
        if !parents.contains(&parent) {
            parents.push(parent);
        }
    }

    pub fn insert_field(
        &mut self,
        owner: impl AsRef<str>,
        name: impl AsRef<str>,
        descriptor: impl AsRef<str>,
        ty: Ty,
        access_flags: u16,
    ) {
        let owner = owner.as_ref().to_string();
        let name = name.as_ref().to_string();
        let descriptor = descriptor.as_ref().to_string();
        self.fields.insert(
            (owner.clone(), name.clone()),
            FieldRef {
                owner,
                name,
                descriptor,
                ty,
                access_flags,
            },
        );
    }

    pub fn insert_method(
        &mut self,
        owner: impl AsRef<str>,
        sig: MethodSig,
        access_flags: u16,
        is_interface: bool,
    ) {
        let owner = owner.as_ref().to_string();
        let descriptor = sig.descriptor();
        let opcode = if is_interface {
            rust_asm::opcodes::INVOKEINTERFACE
        } else {
            rust_asm::opcodes::INVOKEVIRTUAL
        };
        let method = MethodRef {
            owner: owner.clone(),
            name: sig.name.to_string(),
            descriptor,
            return_ty: sig.return_type,
            params: sig.params,
            opcode,
            is_interface,
            is_varargs: access_flags & ACC_VARARGS != 0,
            access_flags,
        };
        self.methods
            .entry((owner, method.name.clone()))
            .or_default()
            .push(method);
    }

    pub fn contains_internal_class(&self, internal_name: &str) -> bool {
        self.classes.contains(internal_name)
    }

    pub fn contains_package(&self, package: &str) -> bool {
        self.packages.contains(package)
    }

    pub fn resolve_import(&self, path: &str, is_wildcard: bool) -> bool {
        let internal_name = path.replace('.', "/");
        if is_wildcard {
            self.contains_package(&internal_name)
        } else {
            self.contains_internal_class(&internal_name)
        }
    }

    pub fn resolve_qualified_name(&self, name: &str) -> Option<&str> {
        let internal_name = name.replace('.', "/");
        self.classes.get(internal_name.as_str()).map(String::as_str)
    }

    pub fn resolve_java_lang(&self, simple_name: &str) -> Option<&str> {
        let internal_name = format!("java/lang/{simple_name}");
        self.classes.get(internal_name.as_str()).map(String::as_str)
    }

    pub fn resolve_simple_name(&self, simple_name: &str) -> Option<&str> {
        match self.simple_names.get(simple_name)? {
            SimpleName::Unique(internal_name) => Some(internal_name.as_str()),
            SimpleName::Ambiguous => None,
        }
    }

    pub fn is_interface(&self, internal_name: &str) -> bool {
        self.interfaces.contains(internal_name)
    }

    pub fn functional_interface_method(&self, internal_name: &str) -> Option<MethodRef> {
        if !self.interfaces.contains(internal_name) {
            return None;
        }

        let mut sam: Option<MethodRef> = None;
        for ((owner, _), methods) in &self.methods {
            if owner == internal_name {
                for m in methods {
                    if m.is_interface {
                        if sam.is_some() {
                            return None;
                        }
                        sam = Some(m.clone());
                    }
                }
            }
        }
        sam
    }

    pub fn resolve_static_field(&self, owner: &str, name: &str) -> Option<FieldRef> {
        self.lookup_order(owner).into_iter().find_map(|owner| {
            self.fields
                .get(&(owner, name.to_string()))
                .filter(|field| field.access_flags & ACC_STATIC != 0)
                .cloned()
        })
    }

    pub fn resolve_instance_method(
        &self,
        receiver: &Ty,
        name: &str,
        args: &[Ty],
    ) -> Option<MethodRef> {
        let owner = match receiver.erasure() {
            Ty::Class(owner) => owner.to_string(),
            Ty::Array(_) => Ty::object().internal_name(),
            _ => return None,
        };
        self.resolve_instance_method_in_hierarchy(&owner, name, args)
    }

    pub fn resolve_static_method(&self, owner: &str, name: &str, args: &[Ty]) -> Option<MethodRef> {
        self.lookup_order(owner)
            .into_iter()
            .filter_map(|owner| self.best_static_method_on_owner(&owner, name, args))
            .min_by(compare_method_matches)
            .map(|candidate| candidate.method)
    }

    pub fn resolve_constructor(&self, owner: &str, args: &[Ty]) -> Option<MethodRef> {
        self.best_instance_method_on_owner(owner, "<init>", args)
            .map(|candidate| candidate.method)
    }

    fn insert_simple_name(&mut self, simple_name: &str, internal_name: &str) {
        match self.simple_names.get(simple_name) {
            Some(SimpleName::Unique(existing)) if existing == internal_name => {}
            Some(_) => {
                self.simple_names
                    .insert(simple_name.to_string(), SimpleName::Ambiguous);
            }
            None => {
                self.simple_names.insert(
                    simple_name.to_string(),
                    SimpleName::Unique(internal_name.to_string()),
                );
            }
        }
    }

    fn resolve_instance_method_in_hierarchy(
        &self,
        owner: &str,
        name: &str,
        args: &[Ty],
    ) -> Option<MethodRef> {
        self.lookup_order(owner)
            .into_iter()
            .filter_map(|owner| self.best_instance_method_on_owner(&owner, name, args))
            .min_by(compare_method_matches)
            .map(|candidate| candidate.method)
    }

    fn best_instance_method_on_owner(
        &self,
        owner: &str,
        name: &str,
        args: &[Ty],
    ) -> Option<MethodCandidate> {
        self.methods
            .get(&(owner.to_string(), name.to_string()))?
            .iter()
            .filter(|method| method.access_flags & ACC_STATIC == 0)
            .filter_map(|method| {
                method_match_score(self, method, args).map(|score| MethodCandidate {
                    method: method.clone(),
                    score,
                })
            })
            .min_by(compare_method_matches)
    }

    fn best_static_method_on_owner(
        &self,
        owner: &str,
        name: &str,
        args: &[Ty],
    ) -> Option<MethodCandidate> {
        self.methods
            .get(&(owner.to_string(), name.to_string()))?
            .iter()
            .filter(|method| method.access_flags & ACC_STATIC != 0)
            .filter_map(|method| {
                method_match_score(self, method, args).map(|score| MethodCandidate {
                    method: method.clone(),
                    score,
                })
            })
            .min_by(compare_method_matches)
    }

    fn lookup_order(&self, owner: &str) -> Vec<String> {
        let mut order = Vec::new();
        let mut seen = HashSet::new();
        let mut queue = VecDeque::from([owner.to_string()]);

        while let Some(current) = queue.pop_front() {
            if !seen.insert(current.clone()) {
                continue;
            }

            order.push(current.clone());
            for parent in self.direct_parents(&current) {
                queue.push_back(parent);
            }
        }

        order
    }

    fn direct_parents(&self, owner: &str) -> Vec<String> {
        let mut parents = self.parents.get(owner).cloned().unwrap_or_default();
        let has_class_parent = parents
            .iter()
            .any(|parent| !self.interfaces.contains(parent));
        if owner != OBJECT_CLASS
            && self.classes.contains(owner)
            && !self.interfaces.contains(owner)
            && !has_class_parent
        {
            parents.push(OBJECT_CLASS.to_string());
        }
        parents
    }
}

#[derive(Clone)]
struct MethodCandidate {
    method: MethodRef,
    score: MethodScore,
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct MethodScore {
    varargs: bool,
    conversion_cost: u16,
}

fn compare_method_matches(left: &MethodCandidate, right: &MethodCandidate) -> Ordering {
    left.score.cmp(&right.score)
}

fn method_match_score(
    catalog: &ClassCatalog,
    method: &MethodRef,
    actual: &[Ty],
) -> Option<MethodScore> {
    if method.is_varargs {
        return varargs_method_score(catalog, method, actual);
    }
    if method.params.len() != actual.len() {
        return None;
    }
    Some(MethodScore {
        varargs: false,
        conversion_cost: conversion_cost(catalog, &method.params, actual)?,
    })
}

fn varargs_method_score(
    catalog: &ClassCatalog,
    method: &MethodRef,
    actual: &[Ty],
) -> Option<MethodScore> {
    let fixed_count = method.params.len().checked_sub(1)?;
    if actual.len() < fixed_count {
        return None;
    }

    if actual.len() == method.params.len()
        && let Some(cost) = conversion_cost(catalog, &method.params, actual)
    {
        return Some(MethodScore {
            varargs: false,
            conversion_cost: cost,
        });
    }

    let Ty::Array(vararg_element) = method.params.last()?.erasure() else {
        return None;
    };
    let fixed_cost = conversion_cost(
        catalog,
        &method.params[..fixed_count],
        &actual[..fixed_count],
    )?;
    let vararg_cost = actual[fixed_count..]
        .iter()
        .map(|arg| conversion_score(catalog, &vararg_element, arg))
        .try_fold(0u16, |sum, cost| sum.checked_add(cost?))?;

    Some(MethodScore {
        varargs: true,
        conversion_cost: fixed_cost + vararg_cost,
    })
}

fn conversion_cost(catalog: &ClassCatalog, expected: &[Ty], actual: &[Ty]) -> Option<u16> {
    expected
        .iter()
        .zip(actual)
        .map(|(expected, actual)| conversion_score(catalog, expected, actual))
        .try_fold(0u16, |sum, score| sum.checked_add(score?))
}

fn conversion_score(catalog: &ClassCatalog, expected: &Ty, actual: &Ty) -> Option<u16> {
    let expected = expected.erasure();
    let actual_is_null = matches!(actual, Ty::Wildcard(None));
    let actual = actual.erasure();
    if expected == actual {
        return Some(0);
    }
    if actual_is_null && matches!(expected, Ty::Class(_) | Ty::Array(_)) {
        return Some(0);
    }
    if expected.is_primitive() && actual.is_primitive() && is_assignable(&actual, &expected) {
        return Some(1);
    }
    if let Some(boxed) = boxing_type(&actual)
        && catalog.is_reference_assignable(&boxed, &expected)
    {
        return Some(2);
    }
    if let Some(unboxed) = unboxing_type(&actual)
        && is_assignable(&unboxed, &expected)
    {
        return Some(2);
    }
    if catalog.is_reference_assignable(&actual, &expected) {
        return Some(3);
    }
    None
}

impl ClassCatalog {
    fn is_reference_assignable(&self, actual: &Ty, expected: &Ty) -> bool {
        match (actual.erasure(), expected.erasure()) {
            (Ty::Class(actual), Ty::Class(expected)) if actual == expected => true,
            (Ty::Class(actual), Ty::Class(expected)) => self
                .lookup_order(actual.as_str())
                .iter()
                .any(|parent| parent == expected.as_str()),
            (Ty::Array(_), Ty::Class(expected)) => expected.as_str() == OBJECT_CLASS,
            (Ty::Array(actual), Ty::Array(expected)) => {
                self.is_reference_assignable(&actual, &expected)
            }
            _ => false,
        }
    }
}

fn normalize_internal_name(name: &str) -> Option<String> {
    let name = name.trim().trim_end_matches(".class");
    if name.is_empty() || name.starts_with('[') || name == "module-info" {
        return None;
    }
    Some(name.replace('.', "/"))
}
