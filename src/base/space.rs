use core::marker::PhantomData;

/// Trait for constant or dynamic workspace handling.
///
/// The method [`workspace()`] is called every time a curve needs space to do calculations on.
///
/// [`workspace()`]: Space::workspace()
pub trait Space<T> {
    // In the fututre with a more powerful type system
    // one may be able to put the definition of T from the trait to the function.
    // However for this to work, we would have to be able to say something like
    // "we will output an array of (any) T", which is not yet easily possible.

    /// The workspace given, this should be an array or a vector (`AsMut<[T]>`)
    type Output: AsMut<[T]>;
    /// Returns the length of the workspace given.
    fn len(&self) -> usize;
    /// Returns true if the workspace is empty.
    fn is_empty(&self) -> bool {
        self.len() == 0
    }
    /// The workspace itself.
    fn workspace(&self) -> Self::Output;
}

/// Struct to handle a constant workspace.
#[derive(Debug, Copy, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct ConstSpace<T, const N: usize> {
    _phantom: PhantomData<fn() -> T>,
}

impl<T, const N: usize> Space<T> for ConstSpace<T, N>
where
    T: Default + Copy,
{
    type Output = [T; N];
    fn len(&self) -> usize {
        N
    }
    fn workspace(&self) -> Self::Output {
        [Default::default(); N]
    }
}

impl<T, const N: usize> ConstSpace<T, N> {
    /// Create a constant worksprace at compile-time.
    pub fn new() -> Self {
        ConstSpace {
            _phantom: PhantomData,
        }
    }
}

impl<T, const N: usize> Default for ConstSpace<T, N> {
    fn default() -> Self {
        Self::new()
    }
}

/// Struct which handles workspace at run-time.
///
/// A new `Vec` is created every time [`workspace()`] is called.
/// This may impact performance as we always allocate memory. However this allows safe concurrency.
///
/// [`workspace()`]: DynSpace::workspace()
#[cfg(feature = "std")]
#[derive(Debug, Copy, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct DynSpace<T> {
    len: usize,
    _phantom: PhantomData<fn() -> T>,
}

#[cfg(feature = "std")]
impl<T> Space<T> for DynSpace<T>
where
    T: Default + Copy,
{
    type Output = Vec<T>;
    fn len(&self) -> usize {
        self.len
    }
    fn workspace(&self) -> Self::Output {
        vec![Default::default(); self.len]
    }
}

#[cfg(feature = "std")]
impl<T> DynSpace<T> {
    /// Create a workspace with given length at run-time.
    pub fn new(len: usize) -> Self {
        DynSpace {
            len,
            _phantom: PhantomData,
        }
    }
}
