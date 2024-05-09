#![no_std]
#![deny(unsafe_op_in_unsafe_fn)]
//! Stable polyfill for [`slice::get_many_mut`].
//!
//! Mostly copied from Rust stdlib core/src/slice.rs

#[cfg(feature = "std")]
extern crate std;

use core::{fmt, mem};

/// This checks every index against each other, and against `len`.
///
/// This will do `binomial(N + 1, 2) = N * (N + 1) / 2 = 0, 1, 3, 6, 10, ..`
/// comparison operations.
fn get_many_check_valid<const N: usize>(
    indices: &[usize; N],
    len: usize,
) -> bool {
    // NB: The optimizer should inline the loops into a sequence
    // of instructions without additional branching.
    let mut valid = true;
    for (i, &idx) in indices.iter().enumerate() {
        valid &= idx < len;
        for &idx2 in &indices[..i] {
            valid &= idx != idx2;
        }
    }
    valid
}

/// Extension trait for [`get_many_mut`](GetManyMutExt::get_many_mut).
pub unsafe trait GetManyMutExt {
    type Element;
    /// Returns mutable references to many indices at once.
    ///
    /// Returns an error if any index is out-of-bounds, or if the same index was
    /// passed more than once.
    ///
    /// # Examples
    ///
    /// ```
    /// use get_many_mut::GetManyMutExt;
    ///
    /// let v = &mut [1, 2, 3];
    /// if let Ok([a, b]) = v.get_many_mut([0, 2]) {
    ///     *a = 413;
    ///     *b = 612;
    /// }
    /// assert_eq!(v, &[413, 2, 612]);
    /// ```
    /// ```should_panic
    /// use get_many_mut::GetManyMutExt;
    ///
    /// let v = &mut [1, 2, 3];
    /// v.get_many_mut([0, 2, 0]).unwrap();
    /// ```
    fn get_many_mut<const N: usize>(
        &mut self,
        indices: [usize; N],
    ) -> Result<[&mut Self::Element; N], GetManyMutError<N>>;
    /// Returns mutable references to many indices at once, without doing any
    /// checks.
    ///
    /// For a safe alternative see [`get_many_mut`].
    ///
    /// # Safety
    ///
    /// Calling this method with overlapping or out-of-bounds indices is
    /// *[undefined behavior]* even if the resulting references are not
    /// used.
    ///
    /// # Examples
    ///
    /// ```
    /// use get_many_mut::GetManyMutExt;
    ///
    /// let x = &mut [1, 2, 4];
    ///
    /// unsafe {
    ///     let [a, b] = x.get_many_unchecked_mut([0, 2]);
    ///     *a *= 10;
    ///     *b *= 100;
    /// }
    /// assert_eq!(x, &[10, 2, 400]);
    /// ```
    ///
    /// [`get_many_mut`]: GetManyMutExt::get_many_mut
    /// [undefined behavior]: https://doc.rust-lang.org/reference/behavior-considered-undefined.html
    unsafe fn get_many_unchecked_mut<const N: usize>(
        &mut self,
        indices: [usize; N],
    ) -> [&mut Self::Element; N];
}

unsafe impl<T> GetManyMutExt for [T] {
    type Element = T;
    fn get_many_mut<const N: usize>(
        &mut self,
        indices: [usize; N],
    ) -> Result<[&mut Self::Element; N], GetManyMutError<N>> {
        if get_many_check_valid(&indices, self.len()) {
            unsafe {
                Ok(<Self as GetManyMutExt>::get_many_unchecked_mut(
                    self, indices,
                ))
            }
        } else {
            Err(GetManyMutError)
        }
    }
    unsafe fn get_many_unchecked_mut<const N: usize>(
        &mut self,
        indices: [usize; N],
    ) -> [&mut T; N] {
        // NB: This implementation is written as it is because any variation of
        // `indices.map(|i| self.get_unchecked_mut(i))` would make miri unhappy,
        // or generate worse code otherwise. This is also why we need to go
        // through a raw pointer here.
        let ptr: *mut T = self.as_mut_ptr();
        let mut arr: mem::MaybeUninit<[&mut T; N]> = mem::MaybeUninit::uninit();
        let arr_ptr: *mut *mut T = arr.as_mut_ptr().cast();

        // SAFETY: We expect `indices` to contain disjunct values that are
        // in bounds of `self`.
        unsafe {
            for i in 0..N {
                let idx = indices[i];
                *arr_ptr.add(i) = &mut *ptr.add(idx);
            }
            arr.assume_init()
        }
    }
}

unsafe impl<T, const M: usize> GetManyMutExt for [T; M] {
    type Element = T;
    fn get_many_mut<const N: usize>(
        &mut self,
        indices: [usize; N],
    ) -> Result<[&mut Self::Element; N], GetManyMutError<N>> {
        <[T] as GetManyMutExt>::get_many_mut(self, indices)
    }
    unsafe fn get_many_unchecked_mut<const N: usize>(
        &mut self,
        indices: [usize; N],
    ) -> [&mut T; N] {
        unsafe { <[T] as GetManyMutExt>::get_many_unchecked_mut(self, indices) }
    }
}

/// The error type returned by
/// [`get_many_mut<N>`][`GetManyMutExt::get_many_mut`].
///
/// It indicates one of two possible errors:
/// - An index is out-of-bounds.
/// - The same index appeared multiple times in the array.
///
/// # Examples
///
/// ```
/// use get_many_mut::GetManyMutExt;
///
/// let v = &mut [1, 2, 3];
/// assert!(v.get_many_mut([0, 999]).is_err());
/// assert!(v.get_many_mut([1, 1]).is_err());
/// ```
// NB: The N here is there to be forward-compatible with adding more details
// to the error type at a later point
#[non_exhaustive]
pub struct GetManyMutError<const N: usize>;

impl<const N: usize> fmt::Debug for GetManyMutError<N> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("GetManyMutError").finish_non_exhaustive()
    }
}

impl<const N: usize> fmt::Display for GetManyMutError<N> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(
            "an index is out of bounds or appeared multiple times in the array",
            f,
        )
    }
}

#[cfg(feature = "std")]
impl<const N: usize> std::error::Error for GetManyMutError<N> {}
