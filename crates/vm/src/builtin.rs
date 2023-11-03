//! Defines the [`Builtin`] trait responsible for executing built-in pre-defined functions.

use crate::error::Error;
use crate::memory::{Segment, Value};

/// An error that occurs when a [`Builtin`] is not able to deduce the value of a memory cell
/// from the given segment.
#[derive(Debug, Clone, Copy)]
pub struct CannotDeduce;

impl From<CannotDeduce> for Error {
    fn from(_value: CannotDeduce) -> Self {
        Error::Builtin
    }
}

/// A built that may be executed by the virtual machine.
pub trait Builtin {
    /// Attempts to deduce the value of a specific memory cell from the given segment.
    ///
    /// # Returns
    ///
    /// If the value could be successfully deduced, `Ok(_)` is returned and the value is written
    /// to `result`.
    ///
    /// Otherwise, [`CannotDeduce`] is returned.
    fn deduce(
        &self,
        offset: usize,
        segment: &Segment,
        result: &mut Value,
    ) -> Result<(), CannotDeduce>;
}
