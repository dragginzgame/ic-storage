//! Identical outer entrypoint meter for both comparison executables.
//! Captures counter zero after the original update returns, including its reply.
use wasm_encoder::{
    CodeSection, ConstExpr, EntityType, ExportKind, ExportSection, Function, FunctionSection,
    GlobalSection, GlobalType, ImportSection, Instruction as I, MemArg, Module, TypeSection,
    ValType,
    reencode::{self, Reencode},
};
use wasmparser::{ExternalKind, Parser, Payload, TypeRef};

type Result<T> = std::result::Result<T, reencode::Error<std::convert::Infallible>>;
struct Meter {
    types: u32,
    imports: Vec<String>,
    functions: u32,
    globals: u32,
    entries: Vec<(String, u32)>,
}

pub(super) fn instrument(bytes: &[u8]) -> Vec<u8> {
    let mut meter = Meter {
        types: 0,
        imports: vec![],
        functions: 0,
        globals: 0,
        entries: vec![],
    };
    for payload in Parser::new(0).parse_all(bytes) {
        match payload.unwrap() {
            Payload::TypeSection(r) => meter.types = r.count(),
            Payload::ImportSection(r) => {
                for import in r.into_imports() {
                    let import = import.unwrap();
                    assert_eq!(import.module, "ic0");
                    assert!(matches!(import.ty, TypeRef::Func(_)));
                    assert_ne!(import.name, "performance_counter");
                    meter.imports.push(import.name.to_owned());
                }
            }
            Payload::FunctionSection(r) => meter.functions = r.count(),
            Payload::GlobalSection(r) => meter.globals = r.count(),
            Payload::ExportSection(r) => {
                for export in r {
                    let export = export.unwrap();
                    assert_ne!(export.name, "canister_query bench_measure");
                    if export.name.starts_with("canister_update bench_") {
                        assert_eq!(export.kind, ExternalKind::Func);
                        meter.entries.push((export.name.into(), export.index));
                    }
                }
            }
            Payload::StartSection { .. } => panic!("no implicit start"),
            _ => {}
        }
    }
    assert!(!meter.entries.is_empty() && meter.globals > 0);
    let mut module = Module::new();
    meter
        .parse_core_module(&mut module, Parser::new(0), bytes)
        .unwrap();
    let result = module.finish();
    ic_storage_retention_probe::artifact::validate_wasm(&result, super::LIMIT).unwrap();
    result
}

impl Meter {
    fn base(&self) -> u32 {
        self.imports.len() as u32 + 1 + self.functions
    }
    fn import(&self, name: &str) -> u32 {
        self.imports.iter().position(|s| s == name).unwrap() as u32
    }
}
impl Reencode for Meter {
    type Error = std::convert::Infallible;
    fn function_index(&mut self, index: u32) -> Result<u32> {
        Ok(index + u32::from(index >= self.imports.len() as u32))
    }
    fn parse_type_section(
        &mut self,
        out: &mut TypeSection,
        r: wasmparser::TypeSectionReader<'_>,
    ) -> Result<()> {
        reencode::utils::parse_type_section(self, out, r)?;
        out.ty().function([ValType::I32], [ValType::I64]);
        out.ty().function([], []);
        Ok(())
    }
    fn parse_import_section(
        &mut self,
        out: &mut ImportSection,
        r: wasmparser::ImportSectionReader<'_>,
    ) -> Result<()> {
        reencode::utils::parse_import_section(self, out, r)?;
        out.import(
            "ic0",
            "performance_counter",
            EntityType::Function(self.types),
        );
        Ok(())
    }
    fn parse_function_section(
        &mut self,
        out: &mut FunctionSection,
        r: wasmparser::FunctionSectionReader<'_>,
    ) -> Result<()> {
        reencode::utils::parse_function_section(self, out, r)?;
        for _ in 0..=self.entries.len() {
            out.function(self.types + 1);
        }
        Ok(())
    }
    fn parse_global_section(
        &mut self,
        out: &mut GlobalSection,
        r: wasmparser::GlobalSectionReader<'_>,
    ) -> Result<()> {
        reencode::utils::parse_global_section(self, out, r)?;
        out.global(
            GlobalType {
                val_type: ValType::I64,
                mutable: true,
                shared: false,
            },
            &ConstExpr::i64_const(0),
        );
        Ok(())
    }
    fn parse_export_section(
        &mut self,
        out: &mut ExportSection,
        r: wasmparser::ExportSectionReader<'_>,
    ) -> Result<()> {
        for export in r {
            let export = export?;
            if let Some(index) = self
                .entries
                .iter()
                .position(|(name, _)| *name == export.name)
            {
                out.export(export.name, ExportKind::Func, self.base() + index as u32);
            } else {
                self.parse_export(out, export)?;
            }
        }
        out.export(
            "canister_query bench_measure",
            ExportKind::Func,
            self.base() + self.entries.len() as u32,
        );
        Ok(())
    }
    fn parse_code_section(
        &mut self,
        out: &mut CodeSection,
        r: wasmparser::CodeSectionReader<'_>,
    ) -> Result<()> {
        reencode::utils::parse_code_section(self, out, r)?;
        for (_, original) in &self.entries {
            assert!(*original >= self.imports.len() as u32);
            let mut body = Function::new([]);
            for instruction in [
                I::Call(original + 1),
                I::I32Const(0),
                I::Call(self.imports.len() as u32),
                I::GlobalSet(self.globals),
                I::End,
            ] {
                body.instruction(&instruction);
            }
            out.function(&body);
        }
        // This query executes no Rust frame. Its scratch writes are discarded
        // by query execution and do not overlap a live owner or retained root.
        let mut body = Function::new([]);
        for instruction in [
            I::I32Const(0),
            I::GlobalGet(self.globals),
            I::I64Store(MemArg {
                offset: 0,
                align: 3,
                target: Default::default(),
                ordering: None,
            }),
            I::I32Const(0),
            I::I32Const(8),
            I::Call(self.import("msg_reply_data_append")),
            I::Call(self.import("msg_reply")),
            I::End,
        ] {
            body.instruction(&instruction);
        }
        out.function(&body);
        Ok(())
    }
}
