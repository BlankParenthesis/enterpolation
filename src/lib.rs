//! This crate provides a myriad of different interpolation, extrapolation and animation methods.
//! Most notably it tries to be generic and modular. If instances of your type act somewhat like
//! a vector space, this crate will be able to interpolate, extrapolate and animate them.
//! TODO: describe more

//TODO: Also our min and max_index of our linear interpolation does NOT clamp the values together...array out of bounds!
//TODO: SO upper_border can be broken somehow! Look into it!

//TODO: all interpolations should have as knots field not just K but Sorted(NonEmpty(K))
//TODO: all interpolations should have as elements field not just E but NonEmpty(E)
//TODO: we want to achieve many different creation options such that a builder will be necessary
//TODO: for now, create a builder for each different interpolation!
//TODO: Afterwards delete the implementation of SortedList for array and vec
//TODO: and add NonEmpty as super trait for SortedList!

#![warn(missing_docs)]

#[macro_use]
extern crate assert_float_eq;

pub mod linear;
pub mod bezier;
pub mod bspline;
pub mod utils;
pub mod homogeneous;

mod real;
mod never;
mod base;

use thiserror::Error;
use crate::real::Real;
pub use base::{Generator, Interpolation, Curve, Extract, Stepper, SortedList, Space,
    DiscreteGenerator, Equidistant, ConstEquidistant, Composite, NonEmpty, Sorted, NonEmptyGenerator, SortedGenerator};
pub use homogeneous::Homogeneous;

/// Struct which chains two Interpolation together to one Interpolation.
///
/// This `struct` is created by [`Interpolation::chain`]. See its documentation for more.
#[derive(Clone, Debug)]
pub struct Chain<A,B>{
    first: A,
    second: B
}

impl<A,B,T> Generator<T> for Chain<A,B>
where
    A: Interpolation<T>,
    B: Interpolation<A::Output>
{
    type Output = B::Output;
    fn gen(&self, scalar: T) -> Self::Output {
        self.second.gen(self.first.gen(scalar))
    }
}

impl<A,B,T> Interpolation<T> for Chain<A,B>
where
    A: Interpolation<T>,
    B: Interpolation<A::Output>
{}

impl<A,B,R> Curve<R> for Chain<A,B>
where
    A: Curve<R>,
    B: Interpolation<A::Output>,
    R: Real,
{
    fn domain(&self) -> [R; 2] {
        self.first.domain()
    }
}

/// The error structure of this crate. Each possible error this crate could return is listed here.
#[derive(Error, Debug)]
pub enum EnterpolationError {
    /// Error returned if the elements given at the creation of an interpolation are to few.
    #[error("To few elements given for creation of `{name}`, {found} elements given, but at least {expected} are necessary.")]
    ToFewElements{
        /// The name of the Interpolation we wanted to create.
        name: String,
        /// The number of elements found.
        found: usize,
        /// The number of elements we need at least.
        expected: usize
    },
    /// Error if the number of knots are not correct at time of creation of an interpolation.
    #[error("The amount of knots given for creation of `{name}` are not correct, {found} knots given, but {expected} necessary.")]
    InvalidNumberKnots{
        /// The name of the Interpolation we wanted to create.
        name: String,
        /// The number of knots found.
        found: usize,
        /// Description how many knots are needed.
        expected: String
    },
}
