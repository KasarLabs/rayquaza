//! Defines the [`Pointer`] type.`

use crate::error::Error;

/// A pointer within a [`Memory`] segment.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Pointer {
    /// The index of the segment in the memory.
    ///
    /// This can be thought of as the provenance of the pointer.
    pub segment: usize,
    /// The offset within the segment.
    pub offset: usize,
}

impl Pointer {
    /// Returns the signed distance between `self` and `other`, given that are refering to the same
    /// segment.
    pub fn subtract(&self, other: &Self) -> Result<isize, Error> {
        if self.segment != other.segment {
            Err(Error::IncoherentProvenance)
        } else {
            Ok(self.offset.wrapping_sub(other.offset) as isize)
        }
    }

    /// Adds `offset` to `self.offset` using wrapping arithmetic.
    #[inline(always)]
    pub fn wrapping_add(self, offset: usize) -> Self {
        Self {
            segment: self.segment,
            offset: self.offset.wrapping_add(offset),
        }
    }

    /// Subtracts `offset` from `self.offset` using wrapping arithmetic.
    #[inline(always)]
    pub fn wrapping_sub(self, offset: usize) -> Self {
        Self {
            segment: self.segment,
            offset: self.offset.wrapping_sub(offset),
        }
    }
}
