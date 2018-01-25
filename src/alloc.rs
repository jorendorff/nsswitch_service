use libc::c_char;
use errors::{Error, Result};
use std::{mem, ptr, slice};
use std::ffi::CStr;
use std::marker::PhantomData;

/// A dead simple memory manager that manages a single contiguous buffer
/// provided by the user. Call `bump_allocator.allocate(value)` to move a value
/// into the buffer.
///
/// The `BumpAllocator` fills up the buffer as you allocate values, in a single
/// left-to-right pass. There is no `free()` operation.
///
/// Once allocated, values in the BumpAllocator are never dropped. So if you
/// move a non-`Copy` value like a `Vec` or `String` into the buffer, it will
/// never get cleaned up: a memory leak.
///
pub struct BumpAllocator<'buf> {
    /// The address of the first unused byte in the buffer.
    point: usize,

    /// The address one byte past the end of the buffer.
    stop: usize,

    /// This field tells the compiler that a BumpAllocator has an exclusive
    /// reference to a buffer; from this, the compiler knows that the
    /// allocator shouldn't outlive the lifetime `'buf`.
    buffer: PhantomData<&'buf mut [u8]>,
}

fn out_of_room<T>() -> Result<T> {
    Err(Error::buffer_too_small())
}

impl<'buf> BumpAllocator<'buf> {
    /// Return a new allocator that carves slices out of the given `buffer`.
    pub fn new(buffer: &'buf mut [u8]) -> BumpAllocator<'buf> {
        BumpAllocator {
            point: buffer.as_ptr() as usize,
            stop: buffer.as_ptr() as usize + buffer.len(),
            buffer: PhantomData
        }
    }

    /// Create a bump allocator that writes to the given fixed-size `buffer`.
    ///
    /// # Safety
    ///
    /// `buffer` must point to `buflen` bytes of uninitialized memory that the
    /// `BumpAllocator` can use for its whole lifetime, even across moves.
    ///
    /// The caller must ensure that neither the `BumpAllocator` nor any
    /// reference into it outlives the buffer. **Rust will not help you enforce
    /// this rule.** Rust has no way of knowing how long the buffer will remain
    /// valid. Consequently it's easy to get Rust to infer an unsafe lifetime
    /// for `'buf` (such that the allocator outlives the buffer), just by
    /// accident.
    pub unsafe fn from_ptr(buffer: *mut c_char, buflen: usize) -> Result<BumpAllocator<'buf>> {
        let point = buffer as usize;
        if buflen > isize::max_value() as usize || buflen > usize::max_value() - point {
            return Err(Error::invalid_args());
        }
        Ok(BumpAllocator::new(slice::from_raw_parts_mut(buffer as *mut u8, buflen)))
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
        let aligned = padded - padded % alignment;
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

    /// Move the given `value` into this allocator's buffer and return a
    /// reference to its new location. This returns an error if there is not
    /// enough room to store `value` with the proper alignment.
    #[allow(dead_code)]
    pub fn allocate<'a, T>(&'a mut self, value: T) -> Result<&'buf mut T> {
        self.align_to::<T>()?;
        let p = self.take(mem::size_of::<T>())? as *mut T;
        unsafe {
            ptr::write(p, value);
            Ok(&mut *p)
        }
    }

    /// Iterate over the given collection, storing its items in a flat array in
    /// the buffer. Returns a pointer to the first element of the array.
    pub fn allocate_array<'a, C: IntoIterator>(
        &'a mut self,
        collection: C
    ) -> Result<&'buf mut [C::Item]> {
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
    pub fn copy_c_str<'a, 'src>(
        &'a mut self,
        str: &'src CStr,
    ) -> Result<&'buf CStr> {
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

#[test]
fn test_alloc() {
    let mut buf = [0_u8; 16];

    // Find a slice of buf that is aligned to an 8-byte boundary.
    let addr = buf.as_ptr() as usize;
    let offset = (8 - addr % 8) % 8;
    assert!((addr + offset) % 8 == 0);

    {
        let mut a = BumpAllocator::new(&mut buf[offset..offset + 8]);

        let r = a.allocate(0x12345678_u32).unwrap();
        assert_eq!(*r, 0x12345678u32);
        assert_eq!((r as *mut u32 as usize) % mem::align_of::<u32>(), 0);

        assert_eq!(*a.allocate(0xfe_u8).unwrap(), 0xfe_u8);

        let r: &mut u16 = a.allocate(0xabcd_u16).unwrap();
        assert_eq!(*r, 0xabcd_u16);
        assert_eq!((r as *mut u16 as usize) % mem::align_of::<u16>(), 0);

        assert!(a.allocate(0xef_u8).is_err());
        assert!(a.allocate(0_u8).is_err());
    }
    assert_eq!((buf[offset + 4], offset), (0xfe, 0));
}

#[test]
fn test_copy_c_str() {
    use std::ffi::CString;

    let mut buf = [0_u8; 100];
    let mut a = BumpAllocator::new(&mut buf);

    let src1 = CString::new("hello world").unwrap();
    let copy1 = a.copy_c_str(&src1).unwrap();
    assert_eq!(copy1.to_str().unwrap(), "hello world");

    let src2 = CString::new("Jello squirreled").unwrap();
    let copy2 = a.copy_c_str(&src2).unwrap();
    assert_eq!(copy2.to_str().unwrap(), "Jello squirreled");

    assert_eq!(copy1.to_str().unwrap(), "hello world");
}
