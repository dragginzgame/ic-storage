//! Closed composition used under the documented single-vector allocation contract.

use std::{borrow::Cow, collections::BTreeMap};

use ic_host_artifacts::{wasm, wasm_text};
use wasm_encoder::{
    CodeSection, CustomSection, ExportSection, FunctionSection, GlobalSection, ImportSection,
    MemorySection, Module, TypeSection, reencode::Reencode,
};
use wasmparser::{ExternalKind, Parser, Payload, TypeRef};

use super::{artifact::LIMIT, vector_access};

type Result<T> = std::result::Result<T, &'static str>;

mod profile;
pub(super) use profile::Profile;

fn checked<T, E>(result: std::result::Result<T, E>) -> Result<T> {
    result.map_err(|_| "vector link encoding")
}

pub(super) fn source(profile: Profile) -> Result<String> {
    profile.check()?;
    let mut source = include_str!("../provider_custody/vector/provider.wat")
        .replace("__VERSION__", &profile.version.to_string())
        .replace("__LENGTH__", &profile.root.0.to_string())
        .replace("__CAPACITY__", &profile.root.1.to_string())
        .replace("__EXTENT__", &profile.extent.to_string())
        .replace("__TRAP__", "0")
        .replace("__FAULT__", &profile.fault.map_or(String::new(), |(offset, value)| {
            format!("(i64.store (i32.const {offset}) (i64.const {value}))")
        }))
        .replace(
            "(func $reply))",
            "(func $reply))\n(import \"vector_link\" \"step\" (func $native_step (param i32 i32 i32 i64) (result i64)))",
        )
        .replace("(func (export \"canister_pre_upgrade\")", "(func $provider_pre")
        .replace("(func (export \"canister_post_upgrade\")", "(func $provider_post");
    let end = source.rfind(')').ok_or("provider closing delimiter")?;
    source.truncate(end);
    source.push_str(include_str!("../provider_custody/vector/access.wat"));
    let mut glue = include_str!("vector_link/entrypoints.wat").replace(
        "__POST_FAILURE__",
        if profile.post_trap {
            "(call $vector_run (i32.const 1) (i64.const 999)) unreachable"
        } else {
            ""
        },
    );
    if !profile.source_guard {
        glue = glue.replace("(export \"canister_pre_upgrade\")", "");
    }
    source.push_str(&glue);
    source.push_str("(export \"__link_claim\" (func $claim))\n");
    for helper in vector_access::HELPERS {
        source.push_str(&format!(
            "(export \"__link_{helper}\" (func $access_{helper}))\n"
        ));
    }
    source.push(')');
    Ok(source)
}

struct Mapping {
    functions: Vec<u32>,
    types: u32,
    globals: u32,
}

impl Reencode for Mapping {
    type Error = &'static str;

    fn function_index(
        &mut self,
        index: u32,
    ) -> std::result::Result<u32, wasm_encoder::reencode::Error<Self::Error>> {
        self.functions
            .get(index as usize)
            .copied()
            .ok_or(wasm_encoder::reencode::Error::UserError(
                "unknown vector function",
            ))
    }

    fn type_index(
        &mut self,
        index: u32,
    ) -> std::result::Result<u32, wasm_encoder::reencode::Error<Self::Error>> {
        Ok(self.types + index)
    }

    fn global_index(
        &mut self,
        index: u32,
    ) -> std::result::Result<u32, wasm_encoder::reencode::Error<Self::Error>> {
        Ok(self.globals + index)
    }
}

pub(super) fn prepare(raw: &[u8], profile: Profile) -> Result<Vec<u8>> {
    let provider = wasm_text::compile(
        &source(profile)?,
        wasm_text::CompileLimits {
            source_bytes: LIMIT,
            module_bytes: LIMIT,
        },
    )
    .map_err(|_| "vector provider source")?;
    compose(raw, &provider)
}

pub(super) fn compose(raw: &[u8], provider: &[u8]) -> Result<Vec<u8>> {
    // Only our admitted raw profile and its deterministic rewrite can enter this
    // composition. An arbitrary "guarded" module is never an accepted input.
    let guarded = vector_access::prepare(raw).map_err(|_| "raw vector profile")?;
    let profile = vector_access::inspect(raw).map_err(|_| "raw vector profile")?;
    wasm::validate(provider, LIMIT).map_err(|_| "vector provider validation")?;
    let mut provider_functions = 0;
    let mut bindings = BTreeMap::new();
    for payload in Parser::new(0).parse_all(provider) {
        match checked(payload)? {
            Payload::FunctionSection(r) => provider_functions = r.count(),
            Payload::ExportSection(r) => {
                for export in r {
                    let export = checked(export)?;
                    if export.name.starts_with("__link_")
                        && (export.kind != ExternalKind::Func
                            || export.index < 3
                            || bindings.insert(export.name, export.index - 1).is_some())
                    {
                        return Err("vector provider binding");
                    }
                }
            }
            _ => {}
        }
    }
    if bindings.len() != 8 {
        return Err("vector binding roster");
    }
    let first_rust = 2 + provider_functions;
    let mut provider_map = Mapping {
        functions: (0..3 + provider_functions)
            .map(|index| match index {
                0 | 1 => index,
                2 => first_rust + profile.entry - 1,
                _ => index - 1,
            })
            .collect(),
        types: 0,
        globals: 0,
    };
    let mut rust_functions = vec![*bindings.get("__link_claim").ok_or("missing claim")?];
    for helper in vector_access::HELPERS {
        rust_functions.push(
            *bindings
                .get(format!("__link_{helper}").as_str())
                .ok_or("missing guard")?,
        );
    }
    rust_functions.extend(first_rust..first_rust + profile.functions);

    let mut types = TypeSection::new();
    let mut imports = ImportSection::new();
    let mut functions = FunctionSection::new();
    let mut memory = MemorySection::new();
    let mut globals = GlobalSection::new();
    let mut exports = ExportSection::new();
    let mut code = CodeSection::new();
    let mut custom = Vec::new();
    for payload in Parser::new(0).parse_all(provider) {
        match checked(payload)? {
            Payload::TypeSection(r) => checked(provider_map.parse_type_section(&mut types, r))?,
            Payload::ImportSection(r) => {
                let mut roster = Vec::new();
                for import in r.into_imports() {
                    let import = checked(import)?;
                    if !matches!(import.ty, TypeRef::Func(_)) {
                        return Err("provider import kind");
                    }
                    roster.push((import.module, import.name));
                    if import.module == "ic0" {
                        checked(provider_map.parse_import(&mut imports, import))?;
                    }
                }
                if roster
                    != [
                        ("ic0", "msg_reply_data_append"),
                        ("ic0", "msg_reply"),
                        ("vector_link", "step"),
                    ]
                {
                    return Err("provider import roster");
                }
            }
            Payload::FunctionSection(r) => {
                checked(provider_map.parse_function_section(&mut functions, r))?
            }
            Payload::MemorySection(r) => {
                checked(provider_map.parse_memory_section(&mut memory, r))?
            }
            Payload::GlobalSection(r) => {
                checked(provider_map.parse_global_section(&mut globals, r))?
            }
            Payload::ExportSection(r) => {
                for export in r {
                    let export = checked(export)?;
                    if !export.name.starts_with("__link_") {
                        checked(provider_map.parse_export(&mut exports, export))?;
                    }
                }
            }
            Payload::CodeSectionEntry(body) => {
                checked(provider_map.parse_function_body(&mut code, body))?
            }
            Payload::CustomSection(section) => {
                // Function-name metadata refers to pre-link indices. Preserve
                // only the independent platform retention marker.
                if section.name() == "icp:private enhanced-orthogonal-persistence" {
                    custom.push(CustomSection {
                        name: Cow::Borrowed(section.name()),
                        data: Cow::Borrowed(section.data()),
                    });
                }
            }
            Payload::Version { .. } | Payload::CodeSectionStart { .. } | Payload::End(_) => {}
            _ => return Err("provider section"),
        }
    }
    let mut rust_map = Mapping {
        functions: rust_functions,
        types: types.len(),
        globals: globals.len(),
    };
    for payload in Parser::new(0).parse_all(&guarded) {
        match checked(payload)? {
            Payload::TypeSection(r) => checked(rust_map.parse_type_section(&mut types, r))?,
            Payload::FunctionSection(r) => {
                checked(rust_map.parse_function_section(&mut functions, r))?
            }
            Payload::GlobalSection(r) => checked(rust_map.parse_global_section(&mut globals, r))?,
            Payload::CodeSectionEntry(body) => {
                checked(rust_map.parse_function_body(&mut code, body))?
            }
            // The rewriter already admitted these; share the provider memory,
            // resolve every import and keep all raw Rust exports private.
            Payload::ImportSection(_)
            | Payload::MemorySection(_)
            | Payload::ExportSection(_)
            | Payload::Version { .. }
            | Payload::CodeSectionStart { .. }
            | Payload::End(_) => {}
            _ => return Err("guarded section"),
        }
    }
    let mut module = Module::new();
    module
        .section(&types)
        .section(&imports)
        .section(&functions)
        .section(&memory)
        .section(&globals)
        .section(&exports)
        .section(&code);
    for section in custom {
        module.section(&section);
    }
    let bytes = module.finish();
    wasm::validate(&bytes, LIMIT).map_err(|_| "combined vector validation")?;
    Ok(bytes)
}

pub(super) fn admit_final(bytes: &[u8], raw: &[u8], profile: Profile) -> Result<()> {
    if bytes != prepare(raw, profile)? {
        return Err("final vector mismatch");
    }
    Ok(())
}

mod installed_tests;
mod tests;
