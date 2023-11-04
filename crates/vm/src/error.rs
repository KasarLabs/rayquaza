//! Defines the [`Error`] type of the crate.

/// An error that might occur when executing a Cairo program.
#[derive(Debug, Clone)]
pub enum Error {
    /// The Cairo VM ran out of physical memory to represent the running program's memory.
    OutOfMemory,
    /// The program counter is pointing to an unknown memory cell, or the memory cell was
    /// known but contained a pointer rather than a scalar value.
    ProgramCounterLost,
    /// The value of the first operand of an instruction could not be deduced from a builtin,
    /// a hint, or a previous assertion.
    CantDeduceOp0,
    /// The value of the second operand of an instruction could not be deduced from a builtin,
    /// a hint, or a previous assertion.
    CantDeduceOp1,
    /// The destination of an instruction could not be deduced from a builtin, a hint, or a
    /// previous assertion.
    CantDeduceDst,
    /// A builtin failed to run correctly because of invalid input.
    Builtin,
    /// Attempted to construct a poitner from a value that cannot be represented within a
    /// the physical memory of the Cairo VM.
    PointerTooLarge,
    /// Attempted to perform an invalid pointer arithmetic operation.
    InvalidPointerArithmetic,
    /// Attempted to divide by zero.
    DivideByZero,
    /// Tried to perform a pointer operation on two pointers that had different provenances.
    IncoherentProvenance,
    /// Attempted to jump to a scalar value with no associated provenance.
    InvalidAbsoluteJump,
    /// Attempted to jump to a pointer value with associated provenance.
    InvalidRelativeJump,
    /// Attempted to return to a scalar value with no associated provenance.
    InvalidReturn,

    /// The value of one of the memory cells contradicted a previous assertion on that same
    /// memory cell.
    ///
    /// This happens with an `AssertEq` instruction is used on a memory cell that has already
    /// been asserted to a different value.
    Contradiction,

    // In most cases, it is recommended to abort the program and return an error to the user.
    //
    /// A memory cell supposed to contain an instruction to executed contained a field element
    /// that did not fit in a 64-bit unsigned integer.
    UndefinedInstruction,
    /// The source of the second operand of an instruction was invalid.
    UndefinedOp1Source,
    /// The result logic of an instruction was invalid.
    UndefinedResultLogic,
    /// The update logic of the **Program Counter** of an instruction was invalid.
    UndefinedPcUpdate,
    /// The update logic of the **Allocation Pointer** of an instruction was invalid.
    UndefinedApUpdate,
    /// The OP code of an instruction was invalid.
    UndefinedOpCode,
    /// In a `Call` instruction, the only allowed `ApUpdate` value is `None`.
    UndefinedApUpdateInCall,
    /// A conditional jump was used with invalid instruction values:
    ///
    /// 1. The result logic was not `Op1`
    /// 2. The op-code was not `None`.
    /// 3. The update logic of the **Allocation Pointer** was not `AddResult`.
    UndefinedConditionalJump,
}
