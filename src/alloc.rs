use libc::c_char;
use errors::{Error, Result};
use std::{mem, ptr, slice};
use std::ffi::CStr;

/// A `BumpAllocator` is a value that can "allocate" smaller pieces from a
/// slab of memory provided by the user. Call `bump_allocator.allocate<T>(value)`
/// to move one value of type T into the buffer.
///
/// Once allocated, values in the BumpAllocator are never dropped.
///
pub struct BumpAllocator {
    /// The address of the first unused byte in the buffer.
    point: usize,

    /// The address one byte past the end of the buffer.
    stop: usize,
}

fn out_of_room<T>() -> Result<T> {
    Err(Error::buffer_too_small())
}

impl BumpAllocator {
    /// Create a bump allocator that writes to the given fixed-size `buffer`.
    ///
    /// `buffer` must point to `buflen` bytes of uninitialized memory that the `BumpAllocator`
    /// can use for its whole lifetime, even across moves.
    pub unsafe fn new(buffer: *mut c_char, buflen: usize) -> Result<BumpAllocator> {
        let point = buffer as usize;
        if buflen > isize::max_value() as usize || buflen > usize::max_value() - point {
            return Err(Error::invalid_args());
        }
        let stop = buffer.offset(buflen as isize) as usize;
        Ok(BumpAllocator { point, stop })
    }

    /// If `self.point` is not properly aligned to hold a value of type `T`,
    /// increment it to an address that is.
    ///
    /// On success, `self.point` is a multiple of `T`'s alignment.
    /// If there is not enough memory left in the buffer to align properly,
    /// then this returns an error, with `self.point` unchanged.
    #[inline]
    fn align_to<T>(&mut self) -> Result<()> {
        // This `match` will be optimized away, since every type's alignment is
        // a constant.
        match mem::align_of::<T>() {
            0 => out_of_room(), // can't happen
            1 => Ok(()),  // nothing to do
            alignment => self.align_to_multiple_of(alignment),
        }
    }

    /// Make `self.point` a multiple of `alignment`, if possible.
    fn align_to_multiple_of(&mut self, alignment: usize) -> Result<()> {
        // Round up to the next multiple of `alignment`, checking carefully for
        // overflow.
        let (padded, wrapped) = self.point.overflowing_add(alignment - 1);
        if wrapped {
            return out_of_room();
        }
        let aligned = padded % alignment;
        if aligned > self.stop {
            return out_of_room();
        }

        self.point = aligned;
        Ok(())
    }

    /// Allocate `nbytes` bytes from this allocator and return the address of
    /// the allocation. This returns an error if `self` has less than `nbytes`
    /// bytes free.
    fn take(&mut self, nbytes: usize) -> Result<usize> {
        if self.stop - self.point < nbytes {
            return out_of_room();
        }
        let p = self.point;
        self.point += nbytes;
        Ok(p)
    }

    /// Move the given `value` into some of this allocator's free space and
    /// return the address. This returns an error if there is not enough room
    /// to store `value` with the proper alignment.
    pub fn allocate<T>(&mut self, value: T) -> Result<&mut T> {
        self.align_to::<T>()?;
        let p = self.take(mem::size_of::<T>())? as *mut T;
        unsafe {
            ptr::write(p, value);
            Ok(&mut *p)
        }
    }

    /// Iterate over the given collection, storing its items in a flat array in
    /// the buffer. Returns a pointer to the first element of the array.
    pub fn allocate_array<C: IntoIterator>(&mut self, collection: C) -> Result<&mut [C::Item]> {
        self.align_to::<C::Item>()?;
        let array_ptr = self.point as *mut C::Item;
        let mut n = 0_usize;
        for value in collection {
            let element_ptr = self.take(mem::size_of::<C::Item>())? as *mut C::Item;
            unsafe {
                ptr::write(element_ptr, value);
            }
            n += 1;
        }
        unsafe {
            debug_assert!(array_ptr.offset(n as isize) as usize == self.point);
            Ok(slice::from_raw_parts_mut(array_ptr, n))
        }
    }

    /// Copy the given null-terminated string into the buffer and return the
    /// address of the copy. This returns an error if there is not enough room
    /// left in the buffer for the whole string, including the trailing NUL
    /// character.
    pub fn copy_c_str<'allocator, 'source>(
        &'allocator mut self,
        str: &'source CStr,
    ) -> Result<&'allocator CStr> {
        let bytes = str.to_bytes_with_nul();
        let src = bytes.as_ptr();
        let nbytes = bytes.len();
        let dst = self.take(nbytes)? as *mut c_char;
        unsafe {
            ptr::copy(src as *const c_char, dst, nbytes);
        }
        self.point += nbytes;
        unsafe {
            Ok(CStr::from_ptr(dst))
        }
    }
}
