//! Instrument a copy of the admitted executable, never the native ABI or data.
//! Counters include instrumentation overhead and end before the final epilogue.
use wasm_encoder::{
    CodeSection, ConstExpr, EntityType, ExportKind, ExportSection, Function, FunctionSection,
    GlobalSection, GlobalType, ImportSection, Instruction as I, MemArg, Module, TypeSection,
    ValType,
    reencode::{self, Reencode},
};
use wasmparser::{ExternalKind, Parser, Payload, TypeRef};

type Result<T> = std::result::Result<T, reencode::Error<std::convert::Infallible>>;
pub(super) const WORDS: usize = 10;
pub(super) const HEADER: &str = "shape\toperation\tpage\tinstructions\twasm_bytes\tstable_bytes\tread_calls\tread_bytes\twrite_calls\twrite_bytes\tgrow_calls\tgrow_requested_pages\tnative_pool_bytes\n";

struct Meter {
    imports: Vec<(String, u32)>,
    types: u32,
    functions: u32,
    globals: u32,
    entries: Vec<(String, u32)>,
}

pub(super) fn instrument(bytes: &[u8]) -> Vec<u8> {
    let mut meter = Meter {
        imports: vec![],
        types: 0,
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
                    let TypeRef::Func(ty) = import.ty else {
                        panic!("function imports only")
                    };
                    assert!(!matches!(
                        import.name,
                        "performance_counter" | "stable_read" | "stable_write" | "stable_grow"
                    ));
                    meter.imports.push((import.name.into(), ty));
                }
            }
            Payload::FunctionSection(r) => meter.functions = r.count(),
            Payload::GlobalSection(r) => meter.globals = r.count(),
            Payload::ExportSection(r) => {
                for export in r {
                    let export = export.unwrap();
                    if export.name.starts_with("canister_") {
                        assert_eq!(export.kind, ExternalKind::Func);
                        assert!(!export.name.contains("measure_"));
                        meter.entries.push((export.name.into(), export.index));
                    }
                }
            }
            Payload::StartSection { .. } => panic!("no implicit start"),
            _ => {}
        }
    }
    for name in [
        "stable64_read",
        "stable64_write",
        "stable64_grow",
        "stable64_size",
        "msg_reply",
        "msg_reply_data_append",
    ] {
        meter.import(name);
    }
    assert!(
        meter
            .entries
            .iter()
            .any(|(name, _)| name == "canister_pre_upgrade")
    );
    let mut module = Module::new();
    meter
        .parse_core_module(&mut module, Parser::new(0), bytes)
        .unwrap();
    let result = module.finish();
    ic_storage_retention_probe::artifact::validate_wasm(&result, super::super::LIMIT).unwrap();
    result
}

fn emit(body: &mut Function, instructions: &[I<'_>]) {
    for instruction in instructions {
        body.instruction(instruction);
    }
}
impl Meter {
    fn import(&self, name: &str) -> u32 {
        self.imports.iter().position(|(n, _)| n == name).unwrap() as u32
    }
    fn base(&self) -> u32 {
        self.imports.len() as u32 + 1 + self.functions
    }
    fn increment(&self, body: &mut Function, field: u32, value: I<'_>) {
        emit(
            body,
            &[
                I::GlobalGet(self.globals + field),
                value,
                I::I64Add,
                I::GlobalSet(self.globals + field),
            ],
        );
    }
    fn entry(&self, original: u32, reply: bool) -> Function {
        let mut body = Function::new([]);
        for field in 0..WORDS as u32 {
            emit(
                &mut body,
                &[I::I64Const(0), I::GlobalSet(self.globals + field)],
            );
        }
        // Exported canister functions are defined, not imports. Internal calls
        // retain their original targets; only the exported boundary is wrapped.
        assert!(original >= self.imports.len() as u32);
        body.instruction(&I::Call(original + 1));
        if reply {
            body.instruction(&I::Call(self.import("msg_reply")));
        }
        emit(
            &mut body,
            &[
                I::MemorySize(0),
                I::I64ExtendI32U,
                I::I64Const(65536),
                I::I64Mul,
                I::GlobalSet(self.globals + 1),
                I::Call(self.import("stable64_size")),
                I::I64Const(65536),
                I::I64Mul,
                I::GlobalSet(self.globals + 2),
                I::I32Const(65560),
                I::I64Load(MemArg {
                    offset: 0,
                    align: 3,
                    target: Default::default(),
                    ordering: None,
                }),
                I::GlobalSet(self.globals + 9),
                I::I32Const(0),
                I::Call(self.imports.len() as u32),
                I::GlobalSet(self.globals),
                I::End,
            ],
        );
        body
    }
}
impl Reencode for Meter {
    type Error = std::convert::Infallible;
    fn function_index(&mut self, index: u32) -> Result<u32> {
        for (offset, name) in ["stable64_read", "stable64_write", "stable64_grow"]
            .into_iter()
            .enumerate()
        {
            if index == self.import(name) {
                return Ok(self.base() + offset as u32);
            }
        }
        Ok(index + u32::from(index >= self.imports.len() as u32))
    }
    fn parse_type_section(
        &mut self,
        types: &mut TypeSection,
        r: wasmparser::TypeSectionReader<'_>,
    ) -> Result<()> {
        reencode::utils::parse_type_section(self, types, r)?;
        types.ty().function([ValType::I32], [ValType::I64]);
        types.ty().function([], []);
        Ok(())
    }
    fn parse_import_section(
        &mut self,
        imports: &mut ImportSection,
        r: wasmparser::ImportSectionReader<'_>,
    ) -> Result<()> {
        reencode::utils::parse_import_section(self, imports, r)?;
        imports.import(
            "ic0",
            "performance_counter",
            EntityType::Function(self.types),
        );
        Ok(())
    }
    fn parse_function_section(
        &mut self,
        functions: &mut FunctionSection,
        r: wasmparser::FunctionSectionReader<'_>,
    ) -> Result<()> {
        reencode::utils::parse_function_section(self, functions, r)?;
        for name in ["stable64_read", "stable64_write", "stable64_grow"] {
            functions.function(self.imports[self.import(name) as usize].1);
        }
        for _ in 0..self.entries.len() + 2 {
            functions.function(self.types + 1);
        }
        Ok(())
    }
    fn parse_global_section(
        &mut self,
        globals: &mut GlobalSection,
        r: wasmparser::GlobalSectionReader<'_>,
    ) -> Result<()> {
        reencode::utils::parse_global_section(self, globals, r)?;
        for _ in 0..WORDS {
            globals.global(
                GlobalType {
                    val_type: ValType::I64,
                    mutable: true,
                    shared: false,
                },
                &ConstExpr::i64_const(0),
            );
        }
        Ok(())
    }
    fn parse_export_section(
        &mut self,
        exports: &mut ExportSection,
        r: wasmparser::ExportSectionReader<'_>,
    ) -> Result<()> {
        for export in r {
            let export = export?;
            if let Some(index) = self
                .entries
                .iter()
                .position(|(name, _)| *name == export.name)
            {
                exports.export(
                    export.name,
                    ExportKind::Func,
                    self.base() + 3 + index as u32,
                );
            } else {
                self.parse_export(exports, export)?;
            }
        }
        exports.export(
            "canister_query measure_costs",
            ExportKind::Func,
            self.base() + 3 + self.entries.len() as u32,
        );
        exports.export(
            "canister_update measure_admission",
            ExportKind::Func,
            self.base() + 4 + self.entries.len() as u32,
        );
        Ok(())
    }
    fn parse_code_section(
        &mut self,
        code: &mut CodeSection,
        r: wasmparser::CodeSectionReader<'_>,
    ) -> Result<()> {
        reencode::utils::parse_code_section(self, code, r)?;
        for (offset, name) in ["stable64_read", "stable64_write", "stable64_grow"]
            .into_iter()
            .enumerate()
        {
            let mut body = Function::new([]);
            let (calls, bytes, args) = if offset == 2 {
                (7, 8, 1)
            } else {
                (3 + offset as u32 * 2, 4 + offset as u32 * 2, 3)
            };
            self.increment(&mut body, calls, I::I64Const(1));
            self.increment(&mut body, bytes, I::LocalGet(args - 1));
            for argument in 0..args {
                body.instruction(&I::LocalGet(argument));
            }
            emit(&mut body, &[I::Call(self.import(name)), I::End]);
            code.function(&body);
        }
        for (_, original) in &self.entries {
            code.function(&self.entry(*original, false));
        }
        // Query scratch is transient and never committed. It runs no Rust frame.
        let mut body = Function::new([]);
        for field in 0..WORDS as u32 {
            emit(
                &mut body,
                &[
                    I::I32Const((field * 8) as i32),
                    I::GlobalGet(self.globals + field),
                    I::I64Store(MemArg {
                        offset: 0,
                        align: 3,
                        target: Default::default(),
                        ordering: None,
                    }),
                ],
            );
        }
        emit(
            &mut body,
            &[
                I::I32Const(0),
                I::I32Const((WORDS * 8) as i32),
                I::Call(self.import("msg_reply_data_append")),
                I::Call(self.import("msg_reply")),
                I::End,
            ],
        );
        code.function(&body);
        let original = self
            .entries
            .iter()
            .find(|(name, _)| name == "canister_pre_upgrade")
            .unwrap()
            .1;
        code.function(&self.entry(original, true));
        Ok(())
    }
}
