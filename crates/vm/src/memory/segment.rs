//! Defines the [`Segment`] type.

use std::alloc::Layout;
use std::fmt;
use std::mem::{align_of, size_of};
use std::ptr::NonNull;

use starknet_types_core::felt::Felt;

use crate::error::Error;

use super::{Pointer, ValueRef};

/// A relocatable segment of memory accessible by the Cairo virtual machine.
///
/// # Representation
///
/// A program running in the Cairo virtual machine is technically allowed to access any value
/// within the address space of the machine. This address space has the size of the Starknet field,
/// which is not realistically representable in a regular computer's memory. Therefore, the Cairo
/// language requires programs to split their memory into *segments*. Each segment is a contiguous
/// block of memory that is located *somewhere* in the virtual machine's address space. The final
/// location of segments is not decided until the program has finished running, meaning that a
/// program can never rely on the final location of a segment.
///
/// This means that a program can never realistically access an arbitrary absolute memory location
/// (since it doesn't know where it is located in the first place). This is good news for us
/// because it means we don't use to deal with *a lot* of fragmentation within individual
/// segments, enabling the use of flat arrays to represent segments. It is still possible for
/// "gaps" to appear within a segment, but they should remain relatively small in most cases.
#[derive(Clone)]
pub struct Segment {
    /// The total capacity of this segment.
    ///
    /// This is the number of memory cells that have been allocated for the segment so far.
    capacity: usize,

    /// The total number of initialized [`Metadata`] entries.
    length: usize,

    /// A pointer to the allocated slice of [`Metadata`] entries.
    ///
    /// All of the entries up to `length` are guaranteed to be initialized.
    metadata: NonNull<Metadata>,

    /// A pointer to the allocated slice of [`Felt`] entries.
    ///
    /// An entry in this array is guaranteed to be initialized if and only if the corresponding
    /// entry in the `metadata` array indicates that the value is `known`.
    cells: NonNull<RawValue>,
}

impl Default for Segment {
    #[inline(always)]
    fn default() -> Self {
        Self::new()
    }
}

impl Segment {
    /// Creates a new empty [`Segment`].
    ///
    /// This function is guaranteed not to fail. In fact, no memory will be allocated by this
    /// function.
    pub const fn new() -> Self {
        Self {
            capacity: 0,
            length: 0,
            metadata: NonNull::dangling(),
            cells: NonNull::dangling(),
        }
    }

    /// Returns the capacity of the segment.
    pub const fn capacity(&self) -> usize {
        self.capacity
    }

    /// Returns the offset of the highest known cell in the segment.
    #[inline(always)]
    pub const fn highest_known_cell(&self) -> usize {
        self.length
    }

    /// Returns the memory cell at offset `index` in the segment, as well as metadata about it.
    ///
    /// # Safety
    ///
    /// The caller must ensure that `index` is within the bounds of the segment's length (i.e. the
    /// offset of the highest known cell).
    unsafe fn get_unchecked_raw(&self, index: usize) -> (&Metadata, &RawValue) {
        // SAFETY:
        //  The caller must ensure that `index` is within the bounds of the segment's length.
        unsafe {
            (
                &*self.metadata.as_ptr().add(index),
                &*self.cells.as_ptr().add(index).cast(),
            )
        }
    }

    /// Returns the memory cell at offset `index` in the segment, as well as metadata about it.
    ///
    /// # Safety
    ///
    /// The caller must ensure that `index` is within the bounds of the segment's length (i.e. the
    /// offset of the highest known cell).
    unsafe fn get_unchecked_raw_mut(&mut self, index: usize) -> (&mut Metadata, &mut RawValue) {
        // SAFETY:
        //  The caller must ensure that `index` is within the bounds of the segment's length.
        unsafe {
            (
                &mut *self.metadata.as_ptr().add(index),
                &mut *self.cells.as_ptr().add(index).cast(),
            )
        }
    }

    /// Returns the memory cell at offset `index` in the segment, if it has been asserted to a
    /// specific value.
    pub fn get(&self, index: usize) -> Option<ValueRef> {
        if index >= self.length {
            None
        } else {
            // SAFETY:
            //  We just made sure that the index is within the bounds of the segment's length.
            let (metadata, cell) = unsafe { self.get_unchecked_raw(index) };

            // SAFETY:
            //  The metadata and its associated cell are guaranteed to be syncronized.
            match *metadata {
                Metadata::Unknown => None,
                Metadata::Pointer => Some(ValueRef::Pointer(unsafe { &cell.pointer })),
                Metadata::Scalar => Some(ValueRef::Scalar(unsafe { &cell.scalar })),
            }
        }
    }

    /// Attempts to assert that a memory cell in the segment has a given value.
    ///
    /// # Returns
    ///
    /// - If it does, the function succeeds and returns `Ok(())`.
    ///
    /// - If it is unknown, the memory cell is asserted to the given value and the function
    ///   succeeds, returning `Ok(())`.
    ///
    /// - If it does not, the function fails and returns `Err(Error::Contradiction)`.
    pub fn assert_eq(&mut self, index: usize, value: ValueRef) -> Result<(), Error> {
        // Ensure that the segment is big enough to store the requested index.
        if index >= self.capacity {
            // Attempt to amortize the cost of growing the segment by growing it by a factor of
            // the current capacity.
            // If the amortized growth is still too small, we grow the segment by the requested
            // index.
            let amortized = self.capacity.saturating_add(4).saturating_mul(3) / 2;
            let min_capacity = index.checked_add(1).ok_or(Error::OutOfMemory)?;
            let new_capacity = amortized.max(min_capacity);

            // SAFETY:
            //  We know that the new capacity will be *at least* `index + 1`, which ensures that
            //  the new capacity is strictly greater than the current capacity (because we know
            //  that `index >= self.capacity`).
            unsafe {
                self.grow(new_capacity)?;
            }
        }

        // If the index is outside of the array's length, write new `Metadata` up to the requested
        // index.
        // If the index is within the array's length, this won't do anything.
        while self.length <= index {
            // SAFETY:
            //  We know that the index is within the bound of our allocated capacity because we
            //  made sure of it earlier in this function.
            unsafe {
                self.metadata
                    .as_ptr()
                    .add(self.length)
                    .write(Metadata::Unknown);
            }

            self.length += 1;
        }

        // SAFETY:
        //  We just made sure that the index is in bounds of the segment's initialized length.
        let (metadata, cell) = unsafe { self.get_unchecked_raw_mut(index) };

        let known = match *metadata {
            Metadata::Unknown => {
                // The cell is unknown.
                // We can assert it to take the provided value.
                *metadata = Metadata::from_value_ref(value);
                cell.write(value);
                return Ok(());
            }
            Metadata::Pointer => ValueRef::Pointer(unsafe { &cell.pointer }),
            Metadata::Scalar => ValueRef::Scalar(unsafe { &cell.scalar }),
        };

        if known != value {
            Err(Error::Contradiction)
        } else {
            Ok(())
        }
    }

    /// Attmepts to grow the capacity of the segment to a given value.
    ///
    /// # Safety
    ///
    /// `new_capacity` must be strictly greater than the current capacity of the segment.
    unsafe fn grow(&mut self, new_capacity: usize) -> Result<(), Error> {
        let new_metadata;
        let new_cells;

        if self.capacity == 0 {
            // The segment is currently empty. In that case, we need to allocate memory for
            // the first time.
            let metadata_layout =
                Layout::array::<Metadata>(new_capacity).map_err(|_| Error::OutOfMemory)?;
            let cells_layout =
                Layout::array::<Felt>(new_capacity).map_err(|_| Error::OutOfMemory)?;

            // SAFETY:
            //  We know by requirements of the function that `new_capacity` is strictly greater
            //  than our current capacity (which is zero), ensuring that it is at least strictly
            //  positive. This ensures that both of those layouts have a strictly positive size.
            unsafe {
                new_metadata = std::alloc::alloc(metadata_layout);
                new_cells = std::alloc::alloc(cells_layout);
            }
        } else {
            // The segment is not currently empty. In that case, we actually need to *reallocate*
            // the memory, moving it to a new location while preserving the existing data.

            unsafe {
                // SAFETY:
                //  Both of those layouts are guaranteed to be valid because they have already
                //  been previously constructed when allocating the memory in the first place.
                let metadata_layout = Layout::from_size_align_unchecked(
                    size_of::<Metadata>().wrapping_mul(self.capacity),
                    align_of::<Metadata>(),
                );
                let cells_layout = Layout::from_size_align_unchecked(
                    size_of::<Felt>().wrapping_mul(self.capacity),
                    align_of::<Felt>(),
                );

                new_metadata = std::alloc::realloc(
                    self.metadata.as_ptr() as *mut u8,
                    metadata_layout,
                    new_capacity,
                );
                new_cells =
                    std::alloc::realloc(self.cells.as_ptr() as *mut u8, cells_layout, new_capacity);
            }
        }

        if new_metadata.is_null() || new_cells.is_null() {
            if !new_cells.is_null() {
                unsafe {
                    // SAFETY:
                    //  This layout has been used to allocate the memory in the first place,
                    //  ensuring that it is valid.
                    let layout = Layout::from_size_align_unchecked(
                        size_of::<Felt>() * new_capacity,
                        align_of::<Felt>(),
                    );

                    // SAFETY:
                    //  We know that this pointer has been allocated previously in this function.
                    std::alloc::dealloc(new_cells, layout);
                }
            }

            if !new_metadata.is_null() {
                unsafe {
                    // SAFETY:
                    //  This layout has been used to allocate the memory in the first place,
                    //  ensuring that it is valid.
                    let layout = Layout::from_size_align_unchecked(
                        size_of::<Metadata>() * new_capacity,
                        align_of::<Metadata>(),
                    );

                    // SAFETY:
                    //  We know that this pointer has been allocated previously in this function.
                    std::alloc::dealloc(new_metadata, layout);
                }
            }

            return Err(Error::OutOfMemory);
        }

        // Everything worked out, we can now update the segment's state.

        self.capacity = new_capacity;

        // SAFETY:
        //  We checked previously in the function that both those pointers were non-null.
        unsafe {
            self.metadata = NonNull::new_unchecked(new_metadata as *mut Metadata);
            self.cells = NonNull::new_unchecked(new_cells as *mut RawValue);
        }

        Ok(())
    }
}

/// A [`Value`] that does not know its disciminant.
union RawValue {
    /// A scalar with no provenance information.
    scalar: Felt,
    /// A pointer with an associated segment.
    pointer: Pointer,
    /// The value is not known yet.
    _unknown: (),
}

impl RawValue {
    /// Creates a new [`RawValue`] from the provided [`ValueRef`] by copying
    /// the referenced value.
    fn write(&mut self, r: ValueRef) {
        match r {
            ValueRef::Scalar(s) => self.scalar = *s,
            ValueRef::Pointer(p) => self.pointer = *p,
        }
    }
}

/// Some metadata kept along memory cells to avoid fragmentation within the array.
///
/// We need to keep metadata separated because a [`Felt`] has a huge alignment of `8` bytes
/// and the metadata we're associating with it is only `1` byte (at least for now). We would
/// be wasting 7 bytes per entry if we were to keep the metadata with the [`Felt`]s.
#[derive(Clone, Debug)]
enum Metadata {
    /// The value of the memory cell is not yet known to the Cairo virtual machine.
    Unknown,
    /// The value of the memroy cell is known to be a pointer with an associated precedence.
    Pointer,
    /// The value of the memory cell is known to be a [`Felt`].
    Scalar,
}

impl Metadata {
    /// Creates a new [`Metadata`] from the provided [`ValueRef`].
    pub fn from_value_ref(v: ValueRef) -> Self {
        match v {
            ValueRef::Scalar(_) => Self::Scalar,
            ValueRef::Pointer(_) => Self::Pointer,
        }
    }
}

impl fmt::Debug for Segment {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Segment").finish_non_exhaustive()
    }
}
