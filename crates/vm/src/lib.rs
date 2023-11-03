//! # Rayquaza
//!
//! A high performance Cairo virtual machine implementation
//!
//! # Documentation
//!
//! - [The Cairo Whitepaper](https://www.cairo-lang.org/cairo-whitepaper/).

#![warn(missing_docs, missing_debug_implementations)]
#![deny(unsafe_op_in_unsafe_fn)]

use std::fmt;

use bitflags::bitflags;
use num_traits::ToPrimitive;
use starknet_types_core::felt::Felt;

use builtin::Builtin;
use cpu::Cpu;
use error::Error;
use instr::{Instruction, ResultLogic};
use memory::{Memory, Pointer, Value};
use trace::Trace;

pub mod builtin;
pub mod cpu;
pub mod error;
pub mod instr;
pub mod memory;
pub mod trace;

/// Contains the full state of a Cairo virtual machine.
///
/// This includes memory, registers, builtins, etc. It can be used to execute a Cairo program
/// and gather execution statistics, traces and other related information.
///
/// # Field
///
/// Technically, the Cairo language allows any prime field to be used as the underlying field
/// for the virtual machine. However, in practice, the only field that this crate is meant to
/// be used is the Starknet field element [`Felt`]. For this reason, it is not possible to change
/// the underlying field of the virtual machine.
///
/// If you need to use another field, feel free to [open an issue](https://github.com/KasarLabs/rayquaza/issues/new).
///
/// # Components
///
/// The [`CairoVM`] is composed of two main components:
///
/// - [`Cpu`]: The central processing unit of the virtual machine, responsible for holding registers
///   and interacting with the memory.
///
/// - [`Memory`]: The memory associated with the virtual machine. Instructions and working memory
///   are stored here.
#[derive(Debug)]
pub struct CairoVM {
    /// The central processing unit of the virtual machine, responsible for holding registers
    /// and interacting with the memory.
    cpu: Cpu,
    /// The memory associated with the virtual machine.
    ///
    /// Instructions and working memory are stored here.
    memory: Memory,

    /// The built-in functions that the virtual machine can execute.
    builtins: BuiltinManager,
}

impl CairoVM {
    /// Returns the current state of the [`Cpu`].
    #[inline(always)]
    pub fn cpu(&self) -> &Cpu {
        &self.cpu
    }

    /// Returns the current state of the [`Memory`].
    #[inline(always)]
    pub fn memory(&self) -> &Memory {
        &self.memory
    }

    /// Advances the virtual machine by a single step, tracing events using the provided [`Trace`]
    /// implementation.
    pub fn step<T>(&mut self, _trace: &mut T) -> Result<(), Error>
    where
        T: ?Sized + Trace,
    {
        // SAFETY:
        //  We make sure when updating the program counter of the `CPU` that the segment it points
        //  to remains valid.
        let instruction = unsafe { fetch_instruction(&self.cpu, &self.memory)? };

        if instruction.is_last_bit_set() {
            return Err(Error::UndefinedInstruction);
        }

        let mut ctx = StepContext::initial(instruction);
        compute_dst(&mut ctx, self);
        compute_op0(&mut ctx, self);
        compute_op1(&mut ctx, self)?;
        run_builtins(&mut ctx, self)?;
        deduce_from_op_code(&mut ctx, self)?;

        Ok(())
    }
}

/// The builtin manager is responsible for holding a collection of [`Builtin`]s implementations
/// and running them when necessary.
///
/// # Segments
///
/// Each builtin is assigned a segment, which is used to store the mapped I/O data that it will
/// used as an input of its execution. The builtin manager is responsible for keeping track
/// of which segment is assigned to which builtin, and efficiently running them when necessary.
struct BuiltinManager {
    /// The first segment allocated for the builtins managed by this [`BuiltinManager`].
    min_segment: usize,
    /// The first segment not allocated for the builtins managed by this [`BuiltinManager`].
    max_segment: usize,
    /// The builtins managed by this [`BuiltinManager`].
    builtins: Box<[Box<dyn Builtin>]>,
}

impl BuiltinManager {
    /// Attempts to get the [`BuiltinRunner`] suitable for deducing a memory cell in the provided
    /// segment.
    pub fn get_runner(&self, segment: usize) -> Option<&dyn Builtin> {
        if segment < self.min_segment || segment >= self.max_segment {
            None
        } else {
            let index = segment - self.min_segment;

            // SAFETY:
            //  We know that `segment` is within the bounds of `self.builtins` because
            //  we checked its value against `self.min_segment` and `self.max_segment`.
            Some(unsafe { &**self.builtins.get_unchecked(index) })
        }
    }
}

impl fmt::Debug for BuiltinManager {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("BuiltinManager")
            .field("min_segment", &self.min_segment)
            .field("max_segment", &self.max_segment)
            .field("builtins", &self.builtins.len())
            .finish()
    }
}

/// Attempts to fetch an instruction from the provided [`Memory`].
///
/// The returned instruction is the one directly referenced by the **Program Counter** of ths
/// [`Cpu`] instance. Note that the instruction is not actually decoded in any way, meaning
/// that it might be missing an eventual associated immediate value.
///
/// # Safety
///
/// The program counter of the [`Cpu`] instance must reference a valid segment within [`Memory`].
#[inline]
unsafe fn fetch_instruction(cpu: &Cpu, memory: &Memory) -> Result<Instruction, Error> {
    // SAFETY:
    //  The caller must make sure that `memory` contains a segment at the index pointed to by
    //  `self.pc.segment`.
    let segment = unsafe { memory.segment_unchecked(cpu.pc.segment) };

    let instr_cell = segment
        .get(cpu.pc.offset)
        .ok_or(Error::ProgramCounterLost)?
        .scalar()
        .ok_or(Error::ProgramCounterLost)?;

    let instr = Instruction(instr_cell.to_u64().ok_or(Error::UndefinedInstruction)?);

    Ok(instr)
}

/// Determines what the destination of an instruction is.
#[inline]
fn compute_dst(ctx: &mut StepContext, vm: &CairoVM) {
    match ctx.instruction.dst_register() {
        instr::DstRegister::AP => ctx.dst_addr = ctx.op0_addr,
        instr::DstRegister::FP => ctx.dst_addr = ctx.op1_addr,
    }

    // We know that this operation won't ever overflow because `ap` and `fp` must
    // both reference values within a segment, which cannot overflow `isize`.
    ctx.dst_addr.offset = ctx
        .dst_addr
        .offset
        .wrapping_add(ctx.instruction.dst_offset() as isize as usize);

    // SAFETY:
    //  We know by invariant of `CairoVM` that the segment referenced by `ap` and `fp`
    //  is always valid.
    let segment = unsafe { vm.memory.segment_unchecked(ctx.dst_addr.segment) };

    if let Some(val) = segment.get(ctx.dst_addr.offset) {
        ctx.dst = val.copied();
        ctx.flags.insert(StepContextFlags::DST_ASSERTED);
    }
}

/// Determines what the first operand of an instruction is.
#[inline]
fn compute_op0(ctx: &mut StepContext, vm: &CairoVM) {
    match ctx.instruction.op0_register() {
        instr::Op0Register::AP => ctx.op0_addr = vm.cpu.ap,
        instr::Op0Register::FP => ctx.op0_addr = vm.cpu.fp,
    }

    // We know that this operation won't ever overflow because `ap` and `fp` must
    // both reference values within a segment, which cannot overflow `isize`.
    ctx.op0_addr.offset = ctx
        .op0_addr
        .offset
        .wrapping_add(ctx.instruction.op0_offset() as isize as usize);

    // SAFETY:
    //  We know by invariant of `CairoVM` that the segment referenced by `ap` and `fp`
    //  is always valid.
    let segment = unsafe { vm.memory.segment_unchecked(ctx.op0_addr.segment) };

    if let Some(val) = segment.get(ctx.op0_addr.offset) {
        ctx.op0 = val.copied();
        ctx.flags.insert(StepContextFlags::OP0_ASSERTED);
    }
}

/// Determines what the second operand of an instruction is.
///
/// This function also updates the `instr_size` field of the provided context.
#[inline]
fn compute_op1(ctx: &mut StepContext, vm: &CairoVM) -> Result<(), Error> {
    match ctx.instruction.op1_source()? {
        instr::Op1Source::Op0 => ctx.op1_addr = ctx.op0_addr,
        instr::Op1Source::PC => {
            ctx.op1_addr = vm.cpu.pc;
            ctx.flags.insert(StepContextFlags::SIZE_TWO);
        }
        instr::Op1Source::FP => ctx.op1_addr = vm.cpu.fp,
        instr::Op1Source::AP => ctx.op1_addr = vm.cpu.ap,
    }

    // We know that this operation won't ever overflow because `ap` and `fp` must
    // both reference values within a segment, which cannot overflow `isize`.
    ctx.op1_addr.offset = ctx
        .op1_addr
        .offset
        .wrapping_add(ctx.instruction.op1_offset() as isize as usize);

    // SAFETY:
    //  We know by invariant of `CairoVM` that the segment referenced by `ap` and `fp`
    //  is always valid.
    let segment = unsafe { vm.memory.segment_unchecked(ctx.op1_addr.segment) };

    if let Some(val) = segment.get(ctx.op1_addr.offset) {
        ctx.op1 = val.copied();
        ctx.flags.insert(StepContextFlags::OP1_ASSERTED);
    }

    Ok(())
}

/// Attempts to deduce the value of a memory cell using one of the registered builtins.
///
/// # Returns
///
/// - `Err(_)` if the choosen builtin failed to run correctly.
///
/// - `Ok(true)` if the value was successfully deduced with a builtin.
///
/// - `Ok(false)` if the value could not be deduced because no builtin was registered for the
///   provided segment.
fn deduce_with_builtin(p: Pointer, vm: &CairoVM, result: &mut Value) -> Result<bool, Error> {
    let Some(runner) = vm.builtins.get_runner(p.segment) else {
        return Ok(false);
    };

    // SAFETY:
    //  We know by invaraint of the `CairoVM` that the segments for which a builtin
    //  is registered are always present.
    let segment = unsafe { vm.memory.segment_unchecked(p.segment) };

    match runner.deduce(p.offset, segment, result) {
        Ok(()) => Ok(true),
        Err(err) => Err(err.into()),
    }
}

/// Runs the builtins when applicable to deduce the missing operands of an instruction.
fn run_builtins(ctx: &mut StepContext, vm: &CairoVM) -> Result<(), Error> {
    if !ctx.flags.has_op0() && deduce_with_builtin(ctx.op0_addr, vm, &mut ctx.op0)? {
        ctx.flags.insert(StepContextFlags::OP0_DEDUCED);
    }

    if !ctx.flags.has_op1() && deduce_with_builtin(ctx.op1_addr, vm, &mut ctx.op1)? {
        ctx.flags.insert(StepContextFlags::OP1_DEDUCED);
    }

    Ok(())
}

/// Attempts to deduce the value of `op1` given a result logic and the values of `op0` and `dst`.
fn deduce_op1_from_op0(
    res_logic: ResultLogic,
    op0: Option<&Value>,
    dst: &Value,
    op1: &mut Value,
) -> Result<bool, Error> {
    match res_logic {
        ResultLogic::Op1 => {
            //    dst = op1
            *op1 = *dst;
            Ok(true)
        }
        ResultLogic::Add => {
            let Some(op0) = op0 else { return Ok(false) };

            //     dst = op0 + op1
            // =>  op1 = dst - op0
            *op1 = dst.subtract(op0)?;
            Ok(true)
        }
        ResultLogic::Mul => {
            let Some(op0) = op0 else { return Ok(false) };

            //     dst = op0 * op1
            // =>  op1 = dst / op0
            *op1 = dst.divide(op0)?;
            Ok(true)
        }
    }
}

/// Attempts to deduce the value of `op0` given a result logic and the values of `op1` and `dst`.
fn deduce_op0_from_op1(
    res_logic: ResultLogic,
    op1: &Value,
    dst: &Value,
    op0: &mut Value,
) -> Result<bool, Error> {
    match res_logic {
        ResultLogic::Op1 => Ok(false),
        ResultLogic::Add => {
            //     dst = op0 + op1
            // =>  op0 = dst - op1
            *op0 = dst.subtract(op1)?;
            Ok(true)
        }
        ResultLogic::Mul => {
            //     dst = op0 * op1
            // =>  op0 = dst / op1
            *op0 = dst.divide(op1)?;
            Ok(true)
        }
    }
}

/// Attempt to deduce missing operands from the OP-Code of the instruction.
fn deduce_from_op_code(ctx: &mut StepContext, vm: &CairoVM) -> Result<(), Error> {
    match ctx.instruction.op_code()? {
        instr::OpCode::Call => {
            // When in a `Call` instruction, `op0`, must be asserted to
            // `pc + instruction_size`.
            let fp_plus_size = vm.cpu.fp.wrapping_add(ctx.flags.instruction_size());
            if ctx.flags.has_op0() {
                if ctx.op0 != fp_plus_size {
                    return Err(Error::Contradiction);
                }
            } else {
                ctx.op0 = fp_plus_size.into();
                ctx.flags.insert(StepContextFlags::OP0_DEDUCED);
            }

            // When in a `Call` instruction, `dst` is asserted to be equal to
            // `fp`.
            if ctx.flags.has_dst() {
                if ctx.dst != vm.cpu.fp {
                    return Err(Error::Contradiction);
                }
            } else {
                ctx.dst = vm.cpu.fp.into();
                ctx.flags.insert(StepContextFlags::DST_DEDUCED);
            }
        }
        instr::OpCode::AssertEq => {
            // With this op-code, we know that the result of the instruction must be
            // asserted to be equal to `dst`.
            // Of course, this is only relevent if we have already have the value of
            // both `dst` and `op0` or `op1`.
            if ctx.flags.has_dst() {
                let res_logic = ctx.instruction.result_logic()?;

                if !ctx.flags.has_op1() {
                    let op0 = if ctx.flags.has_op0() {
                        Some(&ctx.op0)
                    } else {
                        None
                    };

                    // We can deduce `op1`.
                    if deduce_op1_from_op0(res_logic, op0, &ctx.dst, &mut ctx.op1)? {
                        ctx.flags.insert(StepContextFlags::OP1_DEDUCED);
                    }
                }

                if ctx.flags.has_op1() && !ctx.flags.has_op0() {
                    // We can deduce `op0`.
                    if deduce_op0_from_op1(res_logic, &ctx.op1, &ctx.dst, &mut ctx.op0)? {
                        ctx.flags.insert(StepContextFlags::OP0_DEDUCED);
                    }
                }
            }
        }
        _ => (),
    }

    Ok(())
}

bitflags! {
    /// Some flags associated with a [`StepContext`].
    #[derive(Clone, Copy)]
    struct StepContextFlags: u8 {
        /// Whether the destination of the instruction was deduced from the other
        /// operands.
        const DST_DEDUCED = 1 << 0;
        /// Whether the destination of the instruction was asserted by some
        /// already existing memory cell.
        const DST_ASSERTED = 1 << 1;
        /// Whether the first operand of the instruction was deduced from the
        /// other operands.
        const OP0_DEDUCED = 1 << 2;
        /// Whether the first operand of the instruction was asserted by some
        /// already existing memory cell.
        const OP0_ASSERTED = 1 << 3;
        /// Whether the second operand of the instruction was deduced from the
        /// other operands.
        const OP1_DEDUCED = 1 << 4;
        /// Whether the second operand of the instruction was asserted by some
        /// already existing memory cell.
        const OP1_ASSERTED = 1 << 5;
        /// The instruction has a size of two cells instead of one.
        const SIZE_TWO = 1 << 7;
    }
}

impl StepContextFlags {
    /// Returns whether the destination of the instruction is known.
    #[inline(always)]
    pub const fn has_dst(self) -> bool {
        self.contains(Self::DST_ASSERTED.union(Self::DST_DEDUCED))
    }

    /// Returns whether the first operand of the instruction is known.
    #[inline(always)]
    pub const fn has_op0(self) -> bool {
        self.contains(Self::OP0_ASSERTED.union(Self::OP0_DEDUCED))
    }

    /// Returns whether the second operand of the instruction is known.
    #[inline(always)]
    pub const fn has_op1(self) -> bool {
        self.contains(Self::OP1_ASSERTED.union(Self::OP1_DEDUCED))
    }

    /// Returns the size of the instruction in memory cells.
    #[inline(always)]
    pub const fn instruction_size(&self) -> usize {
        if self.contains(Self::SIZE_TWO) {
            2
        } else {
            1
        }
    }
}

/// Stores a state that must be kept around while decoding an instruction.
struct StepContext {
    /// The instruction being decoded.
    pub instruction: Instruction,
    /// The destination address of the instruction being decoded.
    pub dst_addr: Pointer,
    /// The value of the destination of the instruction being decoded, if known.
    ///
    /// Only holds a meaningful value if the `DST_ASSERTED` flag or the `DST_DEDUCED` flag is set.
    pub dst: Value,
    /// The address of the first operand of the instruction being decoded.
    pub op0_addr: Pointer,
    /// The value of the first operand of the instruction being decoded, if known.
    ///
    /// Only holds a meaningful value if the `OP0_ASSERTED` flag or the `OP0_DEDUCED` flag is set.
    pub op0: Value,
    /// The address of the second operand of the instruction being decoded.
    pub op1_addr: Pointer,
    /// The value of the second operand of the instruction being decoded, if known.
    ///
    /// Only holds a meaningful value if the `OP1_ASSERTED` flag or the `OP1_DEDUCED` flag is set.
    pub op1: Value,
    /// Some flags associated with the context.
    pub flags: StepContextFlags,
    /// The next value of the **Frame Pointer**.
    pub next_fp: Pointer,
    /// The next value of the **Allocation Pointer**.
    pub next_ap: Pointer,
    /// The next value of the program counter.
    pub next_pc: Pointer,
}

impl StepContext {
    /// Creates a new [`StepContext`] with the provided instruction.
    ///
    /// All fields are initialized to dummy values and should be properly set before using the
    /// context.
    #[inline]
    pub const fn initial(instruction: Instruction) -> Self {
        Self {
            instruction,
            dst_addr: Pointer {
                segment: 0,
                offset: 0,
            },
            dst: Value::Scalar(Felt::ZERO),
            op0_addr: Pointer {
                segment: 0,
                offset: 0,
            },
            op0: Value::Scalar(Felt::ZERO),
            op1_addr: Pointer {
                segment: 0,
                offset: 0,
            },
            op1: Value::Scalar(Felt::ZERO),
            flags: StepContextFlags::empty(),
            next_fp: Pointer {
                segment: 0,
                offset: 0,
            },
            next_ap: Pointer {
                segment: 0,
                offset: 0,
            },
            next_pc: Pointer {
                segment: 0,
                offset: 0,
            },
        }
    }
}
