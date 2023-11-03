//! Defines the [`Memory`] type, responsible for representing the memory of a Cairo virtual
//! machine.
//!
//! # Immutable Memory
//!
//! Note that the memory of a Cairo virtual machine does not work in the same way regular computer
//! memory do. Instead, it is mathematically immutable, and every operation that would normally
//! mutate memory instead *asserts* a memory cell to a specific value. The value was previously
//! unknown, and now it is defined to a specific value. Any access to that memory cell must now
//! confirm its value is the same as the one it was asserted to.
//!
//! # Segments
//!
//! Each cell of the memory holds an element of a field (in this case, the Starknet field is
//! used). And the total size of the memory is the size of that field. Because it's not possible
//! to represent a field of that size in a regular computer's memory, the Cairo language requires
//! programs to split their memory into *segments*. Each segment is a contiguous block of memory
//! that is located *somewhere* in the virtual machine's address space. The final location of
//! segments is not decided until the program has finished running, meaning that a program can
//! never rely on the final location of a segment.

mod pointer;
mod segment;
mod value;

pub use self::pointer::*;
pub use self::segment::*;
pub use self::value::*;

/// Represents the memory of the Cairo virtual machine.
///
/// More inforamtion on memory can be found in [module-level documentation](self).
#[derive(Default, Debug, Clone)]
pub struct Memory {
    /// The segments that have been initialized in the memory.
    segments: Vec<Segment>,
}

impl Memory {
    /// Returns a [`Segment`] of the memory.
    ///
    /// # Safety
    ///
    /// The provided `segment` must have been allocated previously by this [`Memory`].
    #[inline(always)]
    pub unsafe fn segment_unchecked(&self, segment: usize) -> &Segment {
        unsafe { self.segments.get_unchecked(segment) }
    }

    /// Returns a mutable [`Segment`] of the memory.
    ///
    /// # Safety
    ///
    /// The provided `segment` must have been allocated previously by this [`Memory`].
    #[inline(always)]
    pub unsafe fn segment_unchecked_mut(&mut self, segment: usize) -> &mut Segment {
        unsafe { self.segments.get_unchecked_mut(segment) }
    }
}
