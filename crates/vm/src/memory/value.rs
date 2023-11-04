//! Defines the [`Value`] type.

use num_traits::{ToPrimitive, Zero};
use starknet_types_core::felt::Felt;

use crate::error::Error;

use super::Pointer;

/// A value that may be stored in a [`Memory`] segment.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Value {
    /// A scalar with no provenance information.
    ///
    /// In that case, the value simply carries a specific value without any more information.
    Scalar(Felt),
    /// A pointer with an associated segment.
    ///
    /// In that case, the value is actually a pointer within a specific segment.
    Pointer(Pointer),
}

impl Value {
    /// Attempts to subtract two [`Value`]s.
    pub fn subtract(&self, other: &Self) -> Result<Self, Error> {
        match self {
            Self::Scalar(left) => match other {
                Self::Scalar(right) => Ok(Value::Scalar(left - right)),
                Self::Pointer(_) => Err(Error::InvalidPointerArithmetic),
            },
            Self::Pointer(left) => match other {
                Self::Scalar(right) => match right.to_usize() {
                    Some(right) => Ok(left.wrapping_sub(right).into()),
                    None => Err(Error::PointerTooLarge),
                },
                Self::Pointer(right) => left
                    .subtract(right)
                    .map(|dist| Value::Scalar(Felt::from(dist))),
            },
        }
    }

    /// Attempts to add two [`Value`]s.
    pub fn add(&self, other: &Self) -> Result<Self, Error> {
        match self {
            Self::Scalar(left) => match other {
                Self::Scalar(right) => Ok(Value::Scalar(left + right)),
                Self::Pointer(right) => Ok(right
                    .wrapping_add(left.to_usize().ok_or(Error::PointerTooLarge)?)
                    .into()),
            },
            Self::Pointer(left) => match other {
                Self::Scalar(right) => match right.to_usize() {
                    Some(right) => Ok(left.wrapping_add(right).into()),
                    None => Err(Error::PointerTooLarge),
                },
                Self::Pointer(_) => Err(Error::InvalidPointerArithmetic),
            },
        }
    }

    /// Attempts to divide two values.
    ///
    /// Note that only scalar can be used to divide other values.
    pub fn divide(&self, other: &Self) -> Result<Self, Error> {
        match other {
            Self::Scalar(other) => match other.try_into() {
                Ok(d) => match self {
                    Self::Scalar(n) => Ok(Value::Scalar(n.field_div(&d))),
                    Self::Pointer(_) => Err(Error::InvalidPointerArithmetic),
                },
                Err(_) => Err(Error::DivideByZero),
            },
            Self::Pointer(_) => Err(Error::InvalidPointerArithmetic),
        }
    }

    /// Attempts to multiply two values.
    pub fn multiply(&self, other: &Self) -> Result<Self, Error> {
        match self {
            Self::Scalar(left) => match other {
                Self::Scalar(right) => Ok(Value::Scalar(left * right)),
                Self::Pointer(_) => Err(Error::InvalidPointerArithmetic),
            },
            Self::Pointer(_) => Err(Error::InvalidPointerArithmetic),
        }
    }

    /// Creates a [`ValueRef`] from this [`Value`].
    #[inline(always)]
    pub fn as_ref(&self) -> ValueRef {
        match self {
            Self::Scalar(value) => ValueRef::Scalar(value),
            Self::Pointer(pointer) => ValueRef::Pointer(pointer),
        }
    }

    /// Returns whether this [`Value`] equals zero.
    pub fn is_zero(&self) -> bool {
        match self {
            Self::Scalar(value) => value.is_zero(),
            Self::Pointer(_) => false,
        }
    }
}

impl From<Felt> for Value {
    #[inline(always)]
    fn from(value: Felt) -> Self {
        Self::Scalar(value)
    }
}

impl From<Pointer> for Value {
    #[inline(always)]
    fn from(value: Pointer) -> Self {
        Self::Pointer(value)
    }
}

impl PartialEq<Pointer> for Value {
    fn eq(&self, other: &Pointer) -> bool {
        match self {
            Self::Scalar(_) => false,
            Self::Pointer(pointer) => pointer == other,
        }
    }
}

impl PartialEq<Felt> for Value {
    fn eq(&self, other: &Felt) -> bool {
        match self {
            Self::Scalar(value) => value == other,
            Self::Pointer(_) => false,
        }
    }
}

/// A reference to a [`Value`] that holds the discriminant inline.
///
/// # Notes
///
/// Using a custom reference type is required because:
///
/// 1. We don't want to have to copy a whole `Felt` value every time a memory cell is accessed.
///
/// 2. The memory doesn't directly store `Value`s in memory for layout efficiency reasons,
///    preventing us from creating a reference.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ValueRef<'a> {
    /// A scalar with no provenance information.
    Scalar(&'a Felt),
    /// A pointer with an associated segment.
    Pointer(&'a Pointer),
}

impl<'a> ValueRef<'a> {
    /// Copy the referenced value in a concrete [`Value`] instance.
    #[inline(always)]
    pub const fn copied(self) -> Value {
        match self {
            Self::Scalar(element) => Value::Scalar(*element),
            Self::Pointer(pointer) => Value::Pointer(*pointer),
        }
    }

    /// Attempts to convert the reference to a scalar value.
    #[inline(always)]
    pub const fn scalar(self) -> Option<&'a Felt> {
        match self {
            Self::Scalar(element) => Some(element),
            Self::Pointer(_) => None,
        }
    }

    /// Attempts to convert the reference to a pointer value.
    #[inline(always)]
    pub const fn pointer(self) -> Option<&'a Pointer> {
        match self {
            Self::Scalar(_) => None,
            Self::Pointer(pointer) => Some(pointer),
        }
    }
}
