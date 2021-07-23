//! Builder module for bspline interpolations.
//!
//! Each interpolation has it's own builder module, which accumalates all methods to create their interpolation.

use core::ops::{Mul, Div};
use core::marker::PhantomData;
use num_traits::real::Real;
use num_traits::FromPrimitive;
use num_traits::identities::Zero;
#[cfg(feature = "std")]
use crate::DynSpace;
use crate::{Generator, DiscreteGenerator, Space, ConstSpace, Sorted, NotSorted, SortedGenerator,
    Equidistant, Merge};
use crate::weights::{Weighted, Weights, IntoWeight, Homogeneous};
use crate::builder::{WithWeight,WithoutWeight,Unknown, Type};
use super::BSpline;
use super::error::{BSplineError, InvalidDegree};
use super::adaptors::{BorderBuffer, BorderDeletion};
// use super::error::{LinearError, ToFewElements, KnotElementInequality};

/// Marker struct to signify the building of a closed curve.
#[derive(Debug, Clone, Copy)]
pub struct Clamped;
/// Marker struct to signify the building of an open or generic curve.
#[derive(Debug, Clone, Copy)]
pub struct Open;
/// Marker struct to signify the building of a curve with knots in the usual configuration.
#[derive(Debug, Clone, Copy)]
pub struct Legacy;
// #[derive(Debug, Clone, Copy)]
// pub struct Closed;

/// Marker Struct which saves data for equidistant.
///
/// This struct has len and degree, which are dependent on each other.
/// However the equation between these two variables changes, depending on the specifics of the curve.
/// Such, both should be calculated.
#[derive(Debug, Clone)]
pub struct UnknownDomain<R> {
    _phantom: PhantomData<*const R>,
    len: usize,
    deg: usize,
}

impl<R> UnknownDomain<R>{
    pub fn new(len: usize, deg: usize) -> Self {
        UnknownDomain {
            _phantom: PhantomData,
            len,
            deg,
        }
    }
    pub fn len(&self) -> usize {
        self.len
    }
    pub fn deg(&self) -> usize {
        self.deg
    }
}

/// Builder for bspline interpolation.
///
/// This struct helps create bspline interpolations. The difference between this struct and BSplineBuilder
/// is that this struct is allowed to have fallible methods which are not `build()`.
/// Usually one creates an instance by using the `builder()` method on the interpolation itself.
///
/// Before building, one has to give information for:
/// - The elements the interpolation should use. Methods like `elements` and `elements_with_weights`
/// exist for that cause.
/// - The knots the interpolation uses. Either by giving them directly with `knots` or by using
/// equidistant knots with `equidistant`.
/// - A workspace to use, that is, a mutable slice-like object to do operations on.
/// Usually this is done by calling `constant` or `dynamic`.
#[derive(Debug, Clone)]
pub struct BSplineDirector<K,E,S,W,M> {
    elements: E,
    knots: K,
    space: S,
    _phantoms: (PhantomData<*const W>,PhantomData<*const M>),
}

/// Builder for bspline interpolation.
///
/// This struct helps create bspline interpolations. This is the usual builder to use
/// as the only fallible method is the `build()` method.
/// Usually one creates an instance by using the `builder()` method on the interpolation itself.
///
/// Before building, one has to give information for:
/// - The elements the interpolation should use. Methods like `elements` and `elements_with_weights`
/// exist for that cause.
/// - The knots the interpolation uses. Either by giving them directly with `knots` or by using
/// equidistant knots with `equidistant`.
/// - A workspace to use, that is, a mutable slice-like object to do operations on.
/// Usually this is done by calling `constant` or `dynamic`.
#[derive(Debug, Clone)]
pub struct BSplineBuilder<K,E,S,W,M> {
    inner: Result<BSplineDirector<K,E,S,W,M>,BSplineError>,
}

impl Default for BSplineDirector<Unknown, Unknown, Unknown, Unknown, Open> {
    fn default() -> Self {
        BSplineDirector::new()
    }
}

impl Default for BSplineBuilder<Unknown, Unknown, Unknown, Unknown, Open> {
    fn default() -> Self {
        BSplineBuilder::new()
    }
}

impl BSplineDirector<Unknown, Unknown, Unknown, Unknown, Open> {
    /// Create a new linear interpolation builder.
    pub const fn new() -> Self {
        BSplineDirector {
            elements: Unknown,
            knots: Unknown,
            space: Unknown,
            _phantoms: (PhantomData,PhantomData),
        }
    }
}

impl BSplineBuilder<Unknown, Unknown, Unknown, Unknown, Open> {
    /// Create a new linear interpolation builder.
    pub const fn new() -> Self {
        BSplineBuilder {
            inner: Ok(BSplineDirector::new())
        }
    }
}

impl<M> BSplineDirector<Unknown, Unknown, Unknown, Unknown, M> {

    /// Change the mode to an open curve.
    pub fn open(self) -> BSplineDirector<Unknown, Unknown, Unknown, Unknown, Open> {
        BSplineDirector {
            knots: self.knots,
            space: self.space,
            elements: self.elements,
            _phantoms: (self._phantoms.0,PhantomData),
        }
    }

    /// Change the mode to a clamped curve.
    pub fn clamped(self) -> BSplineDirector<Unknown, Unknown, Unknown, Unknown, Clamped> {
        BSplineDirector {
            knots: self.knots,
            space: self.space,
            elements: self.elements,
            _phantoms: (self._phantoms.0,PhantomData),
        }
    }

    /// Change the mode to legacy.
    ///
    /// This allows the builder to take in the "usual" definition of knots for B-splines.
    /// For more information, see the *Peculariaty of B-splines* section of the [main documentation].
    ///
    /// [main documentation]: enterpolation
    pub fn legacy(self) -> BSplineDirector<Unknown, Unknown, Unknown, Unknown, Legacy> {
        BSplineDirector {
            knots: self.knots,
            space: self.space,
            elements: self.elements,
            _phantoms: (self._phantoms.0,PhantomData),
        }
    }

    // /// Ensure the curve to be a loop, that is, its start and end point are equal and have a smooth transition.
    // ///
    // /// This method changes the underlying knot and element generator, by repeating some.
    // pub fn loop(self) -> BSplineDirector<K,E, Unknown, W>{
    //
    // }

    /// Set the elements of the bspline interpolation.
    pub fn elements<E>(self, elements: E) -> BSplineDirector<Unknown, E, Unknown, WithoutWeight, M>
    where E: DiscreteGenerator,
    {
        BSplineDirector {
            knots: self.knots,
            space: self.space,
            elements,
            _phantoms: (PhantomData,self._phantoms.1),
        }
    }

    /// Set the elements and their weights for this interpolation.
    ///
    /// Weights of `Zero` can achieve unwanted results as their corresponding elements are considered
    /// to be at infinity.
    /// In this case the interpolation may generate NaN, infinite or even panic as elements
    /// are divided by `Zero`.
    ///
    /// If you want to work with points at infinity,
    /// you may want to use homogeneous data itself without this wrapping mechanism.
    ///
    /// # Errors
    ///
    /// Returns `Empty` if the generator is empty.
    pub fn elements_with_weights<G>(self, gen: G)
        -> BSplineDirector<Unknown, Weights<G>,Unknown, WithWeight, M>
    where
        G: DiscreteGenerator,
        G::Output: IntoWeight,
        <G::Output as IntoWeight>::Element:
            Mul<<G::Output as IntoWeight>::Weight, Output = <G::Output as IntoWeight>::Element>,
        <G::Output as IntoWeight>::Weight: Zero + Copy,
    {
        BSplineDirector {
            space: self.space,
            knots: self.knots,
            elements: Weights::new(gen),
            _phantoms: (PhantomData, self._phantoms.1),
        }
    }
}

impl<M> BSplineBuilder<Unknown, Unknown, Unknown, Unknown, M> {

    /// Change the mode to an open curve.
    pub fn open(self) -> BSplineBuilder<Unknown, Unknown, Unknown, Unknown, Open> {
        BSplineBuilder {
            inner: self.inner.and_then(|director| Ok(director.open()))
        }
    }

    /// Change the mode to a clamped curve.
    pub fn clamped(self) -> BSplineBuilder<Unknown, Unknown, Unknown, Unknown, Clamped> {
        BSplineBuilder {
            inner: self.inner.and_then(|director| Ok(director.clamped()))
        }
    }

    /// Change the mode to legacy.
    ///
    /// This allows the builder to take in the "usual" definition of knots for B-splines.
    /// For more information, see the *Peculariaty of B-splines* section of the [main documentation].
    ///
    /// [main documentation]: enterpolation
    pub fn legacy(self) -> BSplineBuilder<Unknown, Unknown, Unknown, Unknown, Legacy> {
        BSplineBuilder {
            inner: self.inner.and_then(|director| Ok(director.legacy()))
        }
    }

    // /// Ensure the curve to be a loop, that is, its start and end point are equal and have a smooth transition.
    // ///
    // /// This method changes the underlying knot and element generator, by repeating some.
    // pub fn loop(self) -> BSplineDirector<K,E, Unknown, W>{
    //
    // }

    /// Set the elements of the bspline interpolation.
    pub fn elements<E>(self, elements: E) -> BSplineBuilder<Unknown, E, Unknown, WithoutWeight, M>
    where E: DiscreteGenerator,
    {
        BSplineBuilder {
            inner: self.inner.and_then(|director| Ok(director.elements(elements)))
        }
    }

    /// Set the elements and their weights for this interpolation.
    ///
    /// Weights of `Zero` can achieve unwanted results as their corresponding elements are considered
    /// to be at infinity.
    /// In this case the interpolation may generate NaN, infinite or even panic as elements
    /// are divided by `Zero`.
    ///
    /// If you want to work with points at infinity,
    /// you may want to use homogeneous data itself without this wrapping mechanism.
    ///
    /// # Errors
    ///
    /// Returns `Empty` if the generator is empty.
    pub fn elements_with_weights<G>(self, gen: G)
        -> BSplineBuilder<Unknown, Weights<G>,Unknown, WithWeight, M>
    where
        G: DiscreteGenerator,
        G::Output: IntoWeight,
        <G::Output as IntoWeight>::Element:
            Mul<<G::Output as IntoWeight>::Weight, Output = <G::Output as IntoWeight>::Element>,
        <G::Output as IntoWeight>::Weight: Zero + Copy,
    {
        BSplineBuilder {
            inner: self.inner.and_then(|director| Ok(director.elements_with_weights(gen)))
        }
    }
}

impl<E,W> BSplineDirector<Unknown, E, Unknown, W, Open>
{
    /// Set the knots of the interpolation.
    ///
    /// The degree of this bspline interplation is given by knots.len() - elements.len() - 1.
    ///
    /// # Errors
    ///
    /// Returns NotSorted if a knot is not greater or equal then the knot before him.
    /// Returns NonValidDegree if the calulated degree would be less than 1.
    ///
    /// # Performance
    ///
    /// If you have equidistant knots, near equidistant knots are you do not really care about
    /// knots, consider using `equidistant()` instead.
    pub fn knots<K>(self, knots: K) -> Result<BSplineDirector<Sorted<K>,E, Unknown, W, Open>, NotSorted>
    where
        E: DiscreteGenerator,
        K: DiscreteGenerator,
        K::Output: PartialOrd
    {
        Ok(BSplineDirector {
            knots: Sorted::new(knots)?,
            elements: self.elements,
            space: self.space,
            _phantoms: self._phantoms,
        })
    }
}

impl<E,W> BSplineBuilder<Unknown, E, Unknown, W, Open>
{
    /// Set the knots of the interpolation.
    ///
    /// The degree of this bspline interplation is given by knots.len() - elements.len() - 1.
    ///
    /// # Errors
    ///
    /// Returns NotSorted if a knot is not greater or equal then the knot before him.
    /// Returns NonValidDegree if the calulated degree would be less than 1.
    ///
    /// # Performance
    ///
    /// If you have equidistant knots, near equidistant knots are you do not really care about
    /// knots, consider using `equidistant()` instead.
    pub fn knots<K>(self, knots: K) -> BSplineBuilder<Sorted<K>,E, Unknown, W, Open>
    where
        E: DiscreteGenerator,
        K: DiscreteGenerator,
        K::Output: PartialOrd
    {
        BSplineBuilder {
            inner: self.inner.and_then(|director| director.knots(knots).map_err(|err| err.into()))
        }
    }
}

impl<E,W> BSplineDirector<Unknown, E, Unknown, W, Clamped>
{
    /// Set the knots of the interpolation.
    ///
    /// The degree of this bspline interplation is given by knots.len() - elements.len() - 1.
    ///
    /// # Errors
    ///
    /// Returns NotSorted if a knot is not greater or equal then the knot before him.
    /// Returns NonValidDegree if the calulated degree would be less than 1.
    ///
    /// # Performance
    ///
    /// If you have equidistant knots, near equidistant knots are you do not really care about
    /// knots, consider using `equidistant()` instead.
    pub fn knots<K>(self, knots: K) -> Result<BSplineDirector<BorderBuffer<Sorted<K>>,E, Unknown, W, Clamped>, BSplineError>
    where
        E: DiscreteGenerator,
        K: DiscreteGenerator,
        K::Output: PartialOrd
    {
        if self.elements.len() < knots.len() {
            return Err(InvalidDegree::new(self.elements.len() as isize - knots.len() as isize +1).into());
        }
        let duplicate = self.elements.len() - knots.len();  //deg-1
        Ok(BSplineDirector {
            knots: BorderBuffer::new(Sorted::new(knots)?, duplicate),
            elements: self.elements,
            space: self.space,
            _phantoms: self._phantoms,
        })
    }
}

impl<E,W> BSplineBuilder<Unknown, E, Unknown, W, Clamped>
{
    /// Set the knots of the interpolation.
    ///
    /// The degree of this bspline interplation is given by knots.len() - elements.len() - 1.
    ///
    /// # Errors
    ///
    /// Returns NotSorted if a knot is not greater or equal then the knot before him.
    /// Returns NonValidDegree if the calulated degree would be less than 1.
    ///
    /// # Performance
    ///
    /// If you have equidistant knots, near equidistant knots are you do not really care about
    /// knots, consider using `equidistant()` instead.
    pub fn knots<K>(self, knots: K) -> BSplineBuilder<BorderBuffer<Sorted<K>>,E, Unknown, W, Clamped>
    where
        E: DiscreteGenerator,
        K: DiscreteGenerator,
        K::Output: PartialOrd
    {
        BSplineBuilder {
            inner: self.inner.and_then(|director| director.knots(knots))
        }
    }
}

impl<E,W> BSplineDirector<Unknown, E, Unknown, W, Legacy>
{
    /// Set the knots of the interpolation.
    ///
    /// The degree of this bspline interplation is given by knots.len() - elements.len() - 1.
    ///
    /// # Panics
    ///
    /// Will panic if the given knots are less than two.
    ///
    /// # Performance
    ///
    /// If you have equidistant knots, near equidistant knots are you do not really care about
    /// knots, consider using `equidistant()` instead.
    pub fn knots<K>(self, knots: K) -> Result<BSplineDirector<BorderDeletion<Sorted<K>>,E, Unknown, W, Legacy>, BSplineError>
    where
        E: DiscreteGenerator,
        K: DiscreteGenerator,
        K::Output: PartialOrd
    {
        Ok(BSplineDirector {
            knots: BorderDeletion::new(Sorted::new(knots)?)?,
            elements: self.elements,
            space: self.space,
            _phantoms: self._phantoms,
        })
    }
}

impl<E,W> BSplineBuilder<Unknown, E, Unknown, W, Legacy>
{
    /// Set the knots of the interpolation.
    ///
    /// The degree of this bspline interplation is given by knots.len() - elements.len() - 1.
    ///
    /// # Panics
    ///
    /// Will panic if the given knots are less than two.
    ///
    /// # Performance
    ///
    /// If you have equidistant knots, near equidistant knots are you do not really care about
    /// knots, consider using `equidistant()` instead.
    pub fn knots<K>(self, knots: K) -> BSplineBuilder<BorderDeletion<Sorted<K>>,E, Unknown, W, Legacy>
    where
        E: DiscreteGenerator,
        K: DiscreteGenerator,
        K::Output: PartialOrd
    {
        BSplineBuilder {
            inner: self.inner.and_then(|director| director.knots(knots))
        }
    }
}

impl<E,W,M> BSplineDirector<Unknown, E, Unknown, W, M>
{
    /// Build an interpolation with equidistant knots.
    pub fn equidistant<R>(self) -> BSplineDirector<Type<R>,E, Unknown, W, M>{
        BSplineDirector {
            knots: Type::new(),
            elements: self.elements,
            space: self.space,
            _phantoms: self._phantoms,
        }
    }
}

impl<E,W,M> BSplineBuilder<Unknown, E, Unknown, W, M>
{
    /// Build an interpolation with equidistant knots.
    pub fn equidistant<R>(self) -> BSplineBuilder<Type<R>,E, Unknown, W, M>{
        BSplineBuilder {
            inner: self.inner.and_then(|director| Ok(director.equidistant()))
        }
    }
}

impl<R,E,W> BSplineDirector<Type<R>, E, Unknown, W, Open>
where
    E: DiscreteGenerator,
{
    /// Set the degree of the curve. The degree has to be bigger than 0 and less than the number of elements,
    /// otherwise it will return an error.
    ///
    /// # Panics
    ///
    /// Panics if the number of elements is zero.
    pub fn degree(self, degree: usize) -> BSplineDirector<UnknownDomain<R>,E,Unknown,W, Open>{
        BSplineDirector{
            knots: UnknownDomain::new(self.elements.len() - 1 + degree, degree),
            elements: self.elements,
            space: self.space,
            _phantoms: self._phantoms,
        }
    }

    /// Set the number of knots.
    ///
    /// For open curves, the number of knots has to be bigger then the number of elements.
    /// For closed curves, the number of knots has to be at most as big as the number of elements.
    ///
    /// # Panic
    ///
    /// Panics if given quantity is less than the number of elements.
    /// May also panic if the number of elements is zero.
    pub fn quantity(self, quantity: usize) -> BSplineDirector<UnknownDomain<R>,E,Unknown,W, Open>{
        BSplineDirector{
            knots: UnknownDomain::new(quantity, quantity - self.elements.len() +1),
            elements: self.elements,
            space: self.space,
            _phantoms: self._phantoms,
        }
    }
}

impl<R,E,W> BSplineBuilder<Type<R>, E, Unknown, W, Open>
where
    E: DiscreteGenerator,
{
    /// Set the degree of the curve. The degree has to be bigger than 0 and less than the number of elements,
    /// otherwise it will return an error.
    ///
    /// # Panics
    ///
    /// Panics if the number of elements is zero.
    pub fn degree(self, degree: usize) -> BSplineBuilder<UnknownDomain<R>,E,Unknown,W, Open>{
        BSplineBuilder {
            inner: self.inner.and_then(|director| Ok(director.degree(degree)))
        }
    }

    /// Set the number of knots.
    ///
    /// For open curves, the number of knots has to be bigger then the number of elements.
    /// For closed curves, the number of knots has to be at most as big as the number of elements.
    ///
    /// # Panic
    ///
    /// Panics if given quantity is less than the number of elements.
    /// May also panic if the number of elements is zero.
    pub fn quantity(self, quantity: usize) -> BSplineBuilder<UnknownDomain<R>,E,Unknown,W, Open>{
        BSplineBuilder {
            inner: self.inner.and_then(|director| Ok(director.quantity(quantity)))
        }
    }
}

impl<R,E,W> BSplineDirector<Type<R>, E, Unknown, W, Clamped>
where
    E: DiscreteGenerator,
{
    /// Set the degree of the curve. The degree has to be bigger than 0 and less than the number of elements,
    /// otherwise it will return an error.
    ///
    /// # Panics
    ///
    /// Panics if the given degree is bigger than the number of elements.
    /// May also panic if the given degree is zero.
    pub fn degree(self, degree: usize) -> BSplineDirector<UnknownDomain<R>,E,Unknown,W, Clamped>{
        BSplineDirector{
            knots: UnknownDomain::new(self.elements.len() - degree + 1, degree),
            elements: self.elements,
            space: self.space,
            _phantoms: self._phantoms,
        }
    }

    /// Set the number of knots.
    ///
    /// For open curves, the number of knots has to be bigger then the number of elements.
    /// For closed curves, the number of knots has to be at most as big as the number of elements.
    ///
    /// # Panics
    ///
    /// Panics if the given quantity is bigger than the number of elements.
    /// May also panic if the given quantity is zero.
    pub fn quantity(self, quantity: usize) -> BSplineDirector<UnknownDomain<R>,E,Unknown,W, Clamped>{
        BSplineDirector{
            knots: UnknownDomain::new(quantity, self.elements.len() - quantity + 1),
            elements: self.elements,
            space: self.space,
            _phantoms: self._phantoms,
        }
    }
}

impl<R,E,W> BSplineBuilder<Type<R>, E, Unknown, W, Clamped>
where
    E: DiscreteGenerator,
{
    /// Set the degree of the curve. The degree has to be bigger than 0 and less than the number of elements,
    /// otherwise it will return an error.
    ///
    /// # Panics
    ///
    /// Panics if the given degree is bigger than the number of elements.
    /// May also panic if the given degree is zero.
    pub fn degree(self, degree: usize) -> BSplineBuilder<UnknownDomain<R>,E,Unknown,W, Clamped>{
        BSplineBuilder{
            inner: self.inner.and_then(|director| Ok(director.degree(degree)))
        }
    }

    /// Set the number of knots.
    ///
    /// For open curves, the number of knots has to be bigger then the number of elements.
    /// For closed curves, the number of knots has to be at most as big as the number of elements.
    ///
    /// # Panics
    ///
    /// Panics if the given quantity is bigger than the number of elements.
    /// May also panic if the given quantity is zero.
    pub fn quantity(self, quantity: usize) -> BSplineBuilder<UnknownDomain<R>,E,Unknown,W, Clamped>{
        BSplineBuilder{
            inner: self.inner.and_then(|director| Ok(director.quantity(quantity)))
        }
    }
}

impl<R,E,W> BSplineDirector<UnknownDomain<R>, E, Unknown, W,Open>
where
    E: DiscreteGenerator,
    R: Real + FromPrimitive,
{
    /// Set the domain of the interpolation.
    pub fn domain(self, start: R, end: R) -> BSplineDirector<Equidistant<R>,E,Unknown,W,Open>{
        BSplineDirector {
            knots: Equidistant::new(self.knots.len(), start, end),
            elements: self.elements,
            space: self.space,
            _phantoms: self._phantoms,
        }
    }

    /// Set the domain of the interpolation to be [0.0,1.0].
    pub fn normalized(self) -> BSplineDirector<Equidistant<R>,E,Unknown,W,Open>{
        BSplineDirector {
            knots: Equidistant::normalized(self.knots.len()),
            elements: self.elements,
            space: self.space,
            _phantoms: self._phantoms,
        }
    }
    /// Set the domain of the interpolation by defining the distance between the knots
    pub fn distance(self, start: R, step: R) -> BSplineDirector<Equidistant<R>,E,Unknown,W,Open>{
        BSplineDirector {
            knots: Equidistant::step(self.knots.len(), start, step),
            elements: self.elements,
            space: self.space,
            _phantoms: self._phantoms,
        }
    }
}

impl<R,E,W> BSplineBuilder<UnknownDomain<R>, E, Unknown, W,Open>
where
    E: DiscreteGenerator,
    R: Real + FromPrimitive,
{
    /// Set the domain of the interpolation.
    pub fn domain(self, start: R, end: R) -> BSplineBuilder<Equidistant<R>,E,Unknown,W,Open>{
        BSplineBuilder {
            inner: self.inner.and_then(|director| Ok(director.domain(start, end)))
        }
    }

    /// Set the domain of the interpolation to be [0.0,1.0].
    pub fn normalized(self) -> BSplineBuilder<Equidistant<R>,E,Unknown,W,Open>{
        BSplineBuilder {
            inner: self.inner.and_then(|director| Ok(director.normalized()))
        }
    }
    /// Set the domain of the interpolation by defining the distance between the knots
    pub fn distance(self, start: R, step: R) -> BSplineBuilder<Equidistant<R>,E,Unknown,W,Open>{
        BSplineBuilder {
            inner: self.inner.and_then(|director| Ok(director.distance(start, step)))
        }
    }
}

impl<R,E,W> BSplineDirector<UnknownDomain<R>, E, Unknown, W, Clamped>
where
    E: DiscreteGenerator,
    R: Real + FromPrimitive,
{
    /// Set the domain of the interpolation.
    pub fn domain(self, start: R, end: R) -> BSplineDirector<BorderBuffer<Equidistant<R>>,E,Unknown,W,Clamped>{
        BSplineDirector {
            knots: BorderBuffer::new(Equidistant::new(self.knots.len(), start, end), self.knots.deg()-1),
            elements: self.elements,
            space: self.space,
            _phantoms: self._phantoms,
        }
    }

    /// Set the domain of the interpolation to be [0.0,1.0].
    pub fn normalized(self) -> BSplineDirector<BorderBuffer<Equidistant<R>>,E,Unknown,W,Clamped>{
        BSplineDirector {
            knots: BorderBuffer::new(Equidistant::normalized(self.knots.len()), self.knots.deg()-1),
            elements: self.elements,
            space: self.space,
            _phantoms: self._phantoms,
        }
    }
    /// Set the domain of the interpolation by defining the distance between the knots
    pub fn distance(self, start: R, step: R) -> BSplineDirector<BorderBuffer<Equidistant<R>>,E,Unknown,W,Clamped>{
        BSplineDirector {
            knots: BorderBuffer::new(Equidistant::step(self.knots.len(), start, step), self.knots.deg()-1),
            elements: self.elements,
            space: self.space,
            _phantoms: self._phantoms,
        }
    }
}

impl<R,E,W> BSplineBuilder<UnknownDomain<R>, E, Unknown, W, Clamped>
where
    E: DiscreteGenerator,
    R: Real + FromPrimitive,
{
    /// Set the domain of the interpolation.
    pub fn domain(self, start: R, end: R) -> BSplineBuilder<BorderBuffer<Equidistant<R>>,E,Unknown,W,Clamped>{
        BSplineBuilder {
            inner: self.inner.and_then(|director| Ok(director.domain(start, end)))
        }
    }

    /// Set the domain of the interpolation to be [0.0,1.0].
    pub fn normalized(self) -> BSplineBuilder<BorderBuffer<Equidistant<R>>,E,Unknown,W,Clamped>{
        BSplineBuilder {
            inner: self.inner.and_then(|director| Ok(director.normalized()))
        }
    }
    /// Set the domain of the interpolation by defining the distance between the knots
    pub fn distance(self, start: R, step: R) -> BSplineBuilder<BorderBuffer<Equidistant<R>>,E,Unknown,W,Clamped>{
        BSplineBuilder {
            inner: self.inner.and_then(|director| Ok(director.distance(start, step)))
        }
    }
}

//TODO: dynamic may return error if elements > knots -> We may want to test this before!

impl<K,E,W,M> BSplineDirector<K,E, Unknown, W,M>
where
    E: DiscreteGenerator,
    K: DiscreteGenerator,
{
    /// Set the workspace which the interpolation uses.
    ///
    /// Tells the builder to use a vector as workspace,
    /// such you don't need to know the degree of the bezier curve at compile-time,
    /// but every generation of a value an allocation of memory will be necessary.
    #[cfg(feature = "std")]
    pub fn dynamic(self) -> BSplineDirector<K,E,DynSpace<E::Output>,W,M>{
        BSplineDirector{
            space: DynSpace::new(self.knots.len() - self.elements.len() + 2),
            knots: self.knots,
            elements: self.elements,
            _phantoms: self._phantoms,
        }
    }

    /// Set the workspace which the interpolation uses.
    ///
    /// Tells the builder the size of the workspace needed such that no memory allocations are needed
    /// when interpolating.
    ///
    /// The size needed is the degree of the interpolation + 1.
    ///
    /// # Errors
    ///
    /// Returns TooSmallWorkspace if the size of the workspace is not at least the degree + 1 of the curve.
    /// To calculate the space needed, one may use the equation: `knots.len() - elements.len()`.
    pub fn constant<const N: usize>(self) -> BSplineDirector<K,E,ConstSpace<E::Output,N>,W,M>
    {
        BSplineDirector{
            knots: self.knots,
            space: ConstSpace::new(),
            elements: self.elements,
            _phantoms: self._phantoms,
        }
    }

    /// Set the workspace whcih the interpolation uses.
    ///
    /// The workspace has to have a size of the number of elements in the bezier curve.
    ///
    /// If the degree of the bezier curve is known at compile-time, consider using `constant` instead.
    /// Otherwise without std support, one has to set a specific object implementing the `Space` trait.
    pub fn workspace<S>(self, space: S) -> BSplineDirector<K,E,S,W,M>
    where S: Space<E::Output>
    {
        BSplineDirector{
            knots: self.knots,
            space,
            elements: self.elements,
            _phantoms: self._phantoms,
        }
    }
}

impl<K,E,W,M> BSplineBuilder<K,E, Unknown, W,M>
where
    E: DiscreteGenerator,
    K: DiscreteGenerator,
{
    /// Set the workspace which the interpolation uses.
    ///
    /// Tells the builder to use a vector as workspace,
    /// such you don't need to know the degree of the bezier curve at compile-time,
    /// but every generation of a value an allocation of memory will be necessary.
    #[cfg(feature = "std")]
    pub fn dynamic(self) -> BSplineBuilder<K,E,DynSpace<E::Output>,W,M>{
        BSplineBuilder {
            inner: self.inner.and_then(|director| Ok(director.dynamic()))
        }
    }

    /// Set the workspace which the interpolation uses.
    ///
    /// Tells the builder the size of the workspace needed such that no memory allocations are needed
    /// when interpolating.
    ///
    /// The size needed is the degree of the interpolation + 1.
    ///
    /// # Errors
    ///
    /// Returns TooSmallWorkspace if the size of the workspace is not at least the degree + 1 of the curve.
    /// To calculate the space needed, one may use the equation: `knots.len() - elements.len()`.
    pub fn constant<const N: usize>(self) -> BSplineBuilder<K,E,ConstSpace<E::Output,N>,W,M>
    {
        BSplineBuilder {
            inner: self.inner.and_then(|director| Ok(director.constant()))
        }
    }

    /// Set the workspace whcih the interpolation uses.
    ///
    /// The workspace has to have a size of the number of elements in the bezier curve.
    ///
    /// If the degree of the bezier curve is known at compile-time, consider using `constant` instead.
    /// Otherwise without std support, one has to set a specific object implementing the `Space` trait.
    pub fn workspace<S>(self, space: S) -> BSplineBuilder<K,E,S,W,M>
    where S: Space<E::Output>
    {
        BSplineBuilder {
            inner: self.inner.and_then(|director| Ok(director.workspace(space)))
        }
    }
}

impl<K,E,S,M> BSplineDirector<K,E,S, WithoutWeight,M>
where
    K: SortedGenerator,
    E: DiscreteGenerator,
    E::Output: Merge<K::Output> + Copy,
    S: Space<E::Output>,
{
    /// Build a bezier interpolation.
    pub fn build(self) -> Result<BSpline<K,E,S>, BSplineError>{
        BSpline::new(self.elements, self.knots, self.space)
    }
}

impl<K,E,S,M> BSplineBuilder<K,E,S, WithoutWeight,M>
where
    K: SortedGenerator,
    E: DiscreteGenerator,
    E::Output: Merge<K::Output> + Copy,
    S: Space<E::Output>,
{
    /// Build a bezier interpolation.
    pub fn build(self) -> Result<BSpline<K,E,S>, BSplineError>{
        match self.inner {
            Err(err) => return Err(err),
            Ok(director) => director.build()
        }
    }
}

impl<K,G,S,M> BSplineDirector<K,Weights<G>,S,WithWeight,M>
where
    G: DiscreteGenerator,
    G::Output: IntoWeight,
    K: SortedGenerator,
    S: Space<Homogeneous<<G::Output as IntoWeight>::Element, <G::Output as IntoWeight>::Weight>>,
    <Weights<G> as Generator<usize>>::Output: Merge<K::Output> + Copy,
    <G::Output as IntoWeight>::Element: Div<<G::Output as IntoWeight>::Weight, Output = <G::Output as IntoWeight>::Element>,
{
    /// Build a weighted bezier interpolation.
    pub fn build(self) -> Result<WeightedBSpline<K,G,S>, BSplineError>
    {
        Ok(Weighted::new(BSpline::new(self.elements, self.knots, self.space)?))
    }
}

impl<K,G,S,M> BSplineBuilder<K,Weights<G>,S,WithWeight,M>
where
    G: DiscreteGenerator,
    G::Output: IntoWeight,
    K: SortedGenerator,
    S: Space<Homogeneous<<G::Output as IntoWeight>::Element, <G::Output as IntoWeight>::Weight>>,
    <Weights<G> as Generator<usize>>::Output: Merge<K::Output> + Copy,
    <G::Output as IntoWeight>::Element: Div<<G::Output as IntoWeight>::Weight, Output = <G::Output as IntoWeight>::Element>,
{
    /// Build a weighted bezier interpolation.
    pub fn build(self) -> Result<WeightedBSpline<K,G,S>, BSplineError>
    {
        match self.inner {
            Err(err) => return Err(err),
            Ok(director) => director.build()
        }
    }
}

/// Type alias for weighted bsplines.
type WeightedBSpline<K,G,S> = Weighted<BSpline<K,Weights<G>,S>>;

#[cfg(test)]
mod test {
    use super::BSplineBuilder;
    // Homogeneous for creating Homogeneous, Generator for using .stack()
    use crate::{weights::Homogeneous, Generator, Curve};

    #[test]
    fn degenerate_creations() {
        let empty : [f64;0] = [];
        assert!(BSplineBuilder::new().elements(empty).knots(empty).constant::<1>().build().is_err());
        assert!(BSplineBuilder::new().elements([1.0]).knots([1.0]).constant::<2>().build().is_err());
    }

    #[test]
    fn mode_equality() {
        let elements = [1.0,3.0,7.0];
        let open = BSplineBuilder::new()
            .elements(elements)
            .knots([0.0,0.0,1.0,1.0])
            .constant::<3>()
            .build().unwrap();
        let clamped = BSplineBuilder::new()
            .clamped()
            .elements(elements)
            .knots([0.0,1.0])
            .constant::<3>()
            .build().unwrap();
        let legacy = BSplineBuilder::new()
            .legacy()
            .elements(elements)
            .knots([0.0,0.0,0.0,1.0,1.0,1.0])
            .constant::<3>()
            .build().unwrap();
            for (a,b,c) in open.take(10)
            .zip(clamped.take(10))
            .zip(legacy.take(10))
            .map(|((a,b),c)| (a,b,c)) {
              assert_f64_near!(a,b);
              assert_f64_near!(b,c);
            }
    }

    #[test]
    fn elements_with_weights() {
        BSplineBuilder::new()
            .elements_with_weights([(1.0,1.0),(2.0,2.0),(3.0,0.0)])
            .equidistant::<f64>()
            .degree(2)
            .domain(0.0,5.0)
            .constant::<3>()
            .build().unwrap();
        BSplineBuilder::new()
            .elements_with_weights([1.0,2.0,3.0].stack([1.0,2.0,0.0]))
            .equidistant::<f64>()
            .degree(1)
            .normalized()
            .constant::<2>()
            .build().unwrap();
        BSplineBuilder::new()
            .elements_with_weights([
                Homogeneous::new(1.0),
                Homogeneous::weighted_unchecked(2.0, 2.0),
                Homogeneous::infinity(3.0)])
            .knots([1.0,2.0,3.0])
            .constant::<2>()
            .build().unwrap();
        BSplineBuilder::new()
            .elements([0.1,0.2,0.3])
            .equidistant::<f64>()
            .degree(1)
            .normalized()
            .constant::<2>()
            .build().unwrap();
    }
}