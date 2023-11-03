//! Defines the [`Cpu`] type, responsible for describing the state of the CPU.
//!
//! More information in the documentation for [`Cpu`].

use crate::memory::Pointer;

/// The Central Processing Unit (CPU) responsible for executing Cairo bytecode instructions.
///
/// But itself, a [`Cpu`] is not enough to execute a Cairo program. In order to do anything
/// useful, it has to be connected to a [`Memory`].
#[derive(Debug, Clone)]
pub struct Cpu {
    /// The Program Counter of the CPU, pointing to the next instruction to be fetched from
    /// working memory.
    ///
    /// It is possible to change the segment in which **PC** points to using an absolute jump,
    /// preventing us from assuming that **PC** is always part of the same segment.
    ///
    /// # Invariants
    ///
    /// The segment in which **PC** points to is always known to be valid within the associated
    /// memory.
    ///
    /// This is of course only the case when the [`Cpu`] is used within a [`CairoVM`](super::CairoVM)
    /// instance.
    pub pc: Pointer,
    /// The Allocation Pointer, incremented by most instructions that need to write to working
    /// memory.
    ///
    /// It is not possible to modify the segment in which **AP** points to, enabling us to assume
    /// that **AP** is always part of the same segment.
    ///
    /// # Invariants
    ///
    /// The segment in which **AP** points to is always known to be valid within the associated
    /// memory.
    ///
    /// This is of course only the case when the [`Cpu`] is used within a [`CairoVM`](super::CairoVM)
    /// instance.
    pub ap: Pointer,
    /// The Frame Pointer, pointing to the base of the current frame.
    ///
    /// Just like the Allocation Pointer, the Frame Pointer cannot change segments.
    ///
    /// # Invariants
    ///
    /// The segment in which **FP** points to is always known to be valid within the associated
    /// memory.
    ///
    /// This is of course only the case when the [`Cpu`] is used within a [`CairoVM`](super::CairoVM)
    /// instance.
    pub fp: Pointer,
}
