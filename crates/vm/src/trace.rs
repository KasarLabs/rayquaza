//! Defines the [`Trace`] trait, used to gather information about the execution of a Cairo
//! program within the virtual machine.

/// A collection of callbacks to be called during the execution of a Cairo program.
#[allow(unused_variables)]
pub trait Trace {}

/// An implementation of [`Trace`] that does nothing.
#[derive(Debug, Default, Clone, Copy)]
pub struct NoopTrace;
impl Trace for NoopTrace {}
