//! Defines the [`Instruction`] type, responsible for representing a single Cairo bytecode
//! instruction, eventually including immediate values.

use std::fmt;

use crate::error::Error;

/// A register the destination part of an instruction can be relative to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum DstRegister {
    /// The **Allocation Pointer**.
    AP = 0,
    /// The **Frame Pointer**.
    FP = 1,
}

/// A register the first operand of an instruction can be relative to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum Op0Register {
    /// The **Allocation Pointer**.
    AP = 0,
    /// The **Frame Pointer**.
    FP = 1,
}

/// A register/object the second operand of an instruction can be relative to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum Op1Source {
    /// The address resulting from the first operand of the instruction.
    Op0 = 0,
    /// The **Program Counter**.
    PC = 1,
    /// The **Frame Pointer**.
    FP = 2,
    /// The **Allocation Pointer**.
    AP = 4,
}

/// A possible result logic to be applied to the first and second operands of an instruction.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum ResultLogic {
    /// The result is simply the value of the second operand.
    Op1 = 0,
    /// The result is the addition of the first and second operands.
    Add = 1,
    /// The result is the multiplication of the first and second operands.
    Mul = 2,
}

/// A possible way to update the **Program Counter** after the instruction has been executed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum PcUpdate {
    /// The size of the instruction is added to the **Program Counter**.
    Regular = 0,
    /// The **Program Counter** is set to the result of the instruction.
    AbsoluteJump = 1,
    /// The result of the instruction is added to the **Program Counter**.
    RelativeJump = 2,
    /// If the destination part of the instruction is zero, then the **Program Counter** is
    /// simply updated according to the [`PcUpdate::Regular`] update rule. Otherwise, the
    /// second part of the instruction is added to it.
    ConditionalJump = 4,
}

/// A possible way to update the **Allocation Pointer** after the instruction has been executed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum ApUpdate {
    /// The **Allocation Pointer** remains unchanged.
    None = 0,
    /// The result of the instruction is added to the **Allocation Pointer**.
    AddResult = 1,
    /// The **Allocation Pointer** is incremented by one.
    Increment = 2,
}

/// The OP code of an instruction.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum OpCode {
    /// The instruction does nothing specific.
    None = 0,
    /// The instruction is calling into a subroutine.
    Call = 1,
    /// The instruction is returning from a subroutine.
    Ret = 2,
    /// The instruction is asserting a specific memory cell to a specific value.
    AssertEq = 4,
}

/// A single Cairo bytecode instruction.
///
/// This contains most of the information required to execute the instruction, but some
/// things might be missing, such as an eventual immediate value.
#[derive(Clone, Copy)]
pub struct Instruction(pub u64);

impl Instruction {
    /// Returns the offset applied to the destination part of the instruction.
    #[inline(always)]
    pub fn dst_offset(&self) -> i16 {
        self.0 as u16 as i16
    }

    /// Returns the offset applied to the first operand of the instruction.
    #[inline(always)]
    pub fn op0_offset(&self) -> i16 {
        (self.0 >> 16) as u16 as i16
    }

    /// Returns the offset applied to the second operand of the instruction.
    #[inline(always)]
    pub fn op1_offset(&self) -> i16 {
        (self.0 >> 32) as u16 as i16
    }

    /// The register that the destination part of the instruction is relative to.
    #[inline(always)]
    pub fn dst_register(&self) -> DstRegister {
        if self.0 & 0x0001_0000_0000_0000 != 0 {
            DstRegister::FP
        } else {
            DstRegister::AP
        }
    }

    /// The register that the first operand of the instruction is relative to.
    #[inline(always)]
    pub fn op0_register(&self) -> Op0Register {
        if self.0 & 0x0002_0000_0000_0000 != 0 {
            Op0Register::FP
        } else {
            Op0Register::AP
        }
    }

    /// The source of the second operand of the instruction.
    #[inline(always)]
    pub fn op1_source(&self) -> Result<Op1Source, Error> {
        match self.0 & 0x001C_0000_0000_0000 {
            0x0000_0000_0000_0000 => Ok(Op1Source::Op0),
            0x0004_0000_0000_0000 => Ok(Op1Source::PC),
            0x0008_0000_0000_0000 => Ok(Op1Source::FP),
            0x0010_0000_0000_0000 => Ok(Op1Source::AP),
            _ => Err(Error::UndefinedOp1Source),
        }
    }

    /// The result logic to be applied to the first and second operands of the instruction.
    #[inline(always)]
    pub fn result_logic(&self) -> Result<ResultLogic, Error> {
        match self.0 & 0x0060_0000_0000_0000 {
            0x0000_0000_0000_0000 => Ok(ResultLogic::Op1),
            0x0020_0000_0000_0000 => Ok(ResultLogic::Add),
            0x0040_0000_0000_0000 => Ok(ResultLogic::Mul),
            _ => Err(Error::UndefinedResultLogic),
        }
    }

    /// Returns the update rule to be applied to the **Program Counter** after the instruction
    #[inline(always)]
    pub fn pc_update(&self) -> Result<PcUpdate, Error> {
        match self.0 & 0x0380_0000_0000_0000 {
            0x0000_0000_0000_0000 => Ok(PcUpdate::Regular),
            0x0080_0000_0000_0000 => Ok(PcUpdate::AbsoluteJump),
            0x0100_0000_0000_0000 => Ok(PcUpdate::RelativeJump),
            0x0200_0000_0000_0000 => Ok(PcUpdate::ConditionalJump),
            _ => Err(Error::UndefinedPcUpdate),
        }
    }

    /// Returns the update rule to be applied to the **Allocation Pointer** after the instruction
    #[inline(always)]
    pub fn ap_update(&self) -> Result<ApUpdate, Error> {
        match self.0 & 0x0C00_0000_0000_0000 {
            0x0000_0000_0000_0000 => Ok(ApUpdate::None),
            0x0400_0000_0000_0000 => Ok(ApUpdate::AddResult),
            0x0800_0000_0000_0000 => Ok(ApUpdate::Increment),
            _ => Err(Error::UndefinedApUpdate),
        }
    }

    /// Returns the OP code of the instruction.
    #[inline(always)]
    pub fn op_code(&self) -> Result<OpCode, Error> {
        match self.0 & 0xF000_0000_0000_0000 {
            0x0000_0000_0000_0000 => Ok(OpCode::None),
            0x1000_0000_0000_0000 => Ok(OpCode::Call),
            0x2000_0000_0000_0000 => Ok(OpCode::Ret),
            0x4000_0000_0000_0000 => Ok(OpCode::AssertEq),
            _ => Err(Error::UndefinedOpCode),
        }
    }

    /// Returns whether the last bit of the instruction representation is set or not.
    ///
    /// Normally, a properly 0 instruction should have this bit set to zero.
    #[inline(always)]
    pub fn is_last_bit_set(&self) -> bool {
        self.0 & 0x8000_0000_0000_0000 != 0
    }
}

impl fmt::Debug for Instruction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Instruction")
            .field("dst_offset", &self.dst_offset())
            .field("op0_offset", &self.op0_offset())
            .field("op1_offset", &self.op1_offset())
            .field("dst_register", &self.dst_register())
            .field("op0_register", &self.op0_register())
            .field("op1_source", &self.op1_source())
            .field("result_logic", &self.result_logic())
            .field("pc_update", &self.pc_update())
            .field("ap_update", &self.ap_update())
            .field("op_code", &self.op_code())
            .finish()
    }
}
