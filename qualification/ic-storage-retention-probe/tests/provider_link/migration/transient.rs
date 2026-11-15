//! Rebuild the entire discarded checkpoint domain before entering its new Rust.
//!
//! This is not private allocator-layout admission: no old allocation survives.
//! Callers must first admit the native root and end every native Rust frame.

use wasm_encoder::{DataSection, Function, Instruction as I};
use wasmparser::{DataKind, ExternalKind, Operator as Op, Parser, Payload, ValType};

pub(super) struct Plan {
    stack: u32,
    segments: Vec<(i32, Vec<u8>)>,
}

impl Plan {
    /// Additional admission for the exact compiler domain already inventoried
    /// by the composer. Unknown mutable state or dynamic initialization refuses.
    pub(super) fn read(bytes: &[u8]) -> Self {
        let mut stack = None;
        let mut mutable = Vec::new();
        let mut segments = Vec::new();
        for payload in Parser::new(0).parse_all(bytes) {
            match payload.unwrap() {
                Payload::GlobalSection(r) => {
                    for (index, global) in r.into_iter().enumerate() {
                        let global = global.unwrap();
                        if global.ty.mutable {
                            assert_eq!(global.ty.content_type, ValType::I32);
                            let mut ops = global.init_expr.get_operators_reader();
                            assert!(matches!(ops.read().unwrap(), Op::I32Const { value: 65536 }));
                            assert!(matches!(ops.read().unwrap(), Op::End) && ops.eof());
                            mutable.push(index as u32);
                        }
                    }
                }
                Payload::ExportSection(r) => {
                    for export in r {
                        let export = export.unwrap();
                        if export.name == "__stack_pointer" {
                            assert_eq!(export.kind, ExternalKind::Global);
                            stack = Some(export.index);
                        }
                    }
                }
                Payload::DataSection(r) => {
                    for data in r {
                        let data = data.unwrap();
                        let DataKind::Active {
                            memory_index: 0,
                            offset_expr,
                        } = data.kind
                        else {
                            panic!("reset requires active initialization data")
                        };
                        let mut ops = offset_expr.get_operators_reader();
                        let Op::I32Const { value } = ops.read().unwrap() else {
                            panic!("constant initialization offset")
                        };
                        assert!(matches!(ops.read().unwrap(), Op::End) && ops.eof());
                        assert!(value >= 196608);
                        segments.push((value, data.data.to_vec()));
                    }
                }
                Payload::CodeSectionEntry(body) => {
                    for op in body.get_operators_reader().unwrap() {
                        assert!(
                            !matches!(
                                op.unwrap(),
                                Op::MemoryInit { .. }
                                    | Op::DataDrop { .. }
                                    | Op::TableSet { .. }
                                    | Op::TableGrow { .. }
                                    | Op::TableFill { .. }
                                    | Op::TableCopy { .. }
                                    | Op::TableInit { .. }
                                    | Op::ElemDrop { .. }
                            ),
                            "dynamic data/table state is outside reset admission"
                        );
                    }
                }
                _ => {}
            }
        }
        let stack = stack.unwrap();
        assert_eq!(mutable, [stack], "unreviewed mutable compiler globals");
        assert!(!segments.is_empty());
        Self { stack, segments }
    }

    /// Preserve active initialization for install/recovery; private passive
    /// copies supply the target's pristine data when reopening a retained heap.
    pub(super) fn append(self, data: &mut DataSection) -> Function {
        let mut reset = Function::new([]);
        // Physical memory never shrinks. Include every grown page in the reset;
        // the admitted 256-page maximum makes the byte arithmetic non-wrapping.
        reset.instruction(&I::I32Const(196608));
        reset.instruction(&I::I32Const(0));
        reset.instruction(&I::MemorySize(0));
        reset.instruction(&I::I32Const(65536));
        reset.instruction(&I::I32Mul);
        reset.instruction(&I::I32Const(196608));
        reset.instruction(&I::I32Sub);
        reset.instruction(&I::MemoryFill(0));
        reset.instruction(&I::I32Const(0));
        reset.instruction(&I::I32Const(0));
        reset.instruction(&I::I32Const(65536));
        reset.instruction(&I::MemoryFill(0));
        for (offset, bytes) in self.segments {
            let index = data.len();
            reset.instruction(&I::I32Const(offset));
            reset.instruction(&I::I32Const(0));
            reset.instruction(&I::I32Const(bytes.len() as i32));
            reset.instruction(&I::MemoryInit {
                mem: 0,
                data_index: index,
            });
            data.passive(bytes);
        }
        reset.instruction(&I::I32Const(65536));
        reset.instruction(&I::GlobalSet(self.stack));
        reset.instruction(&I::End);
        reset
    }
}
