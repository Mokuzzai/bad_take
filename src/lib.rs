

use std::marker::PhantomData;
use std::ops::Deref;
use std::ops::DerefMut;
use std::ptr::NonNull;

pub trait IterTakeExt {
	type Item;
	/// # Basic usage
	///
	/// Creates an mutable iterator over [`Vec<T>`] that allows items to be removed while iterating
	///
	/// ```
	/// use bad_take::IterTakeExt;
	/// use bad_take::Take;
	///
	/// let mut numbers = vec![1, 2, 3, 4, 5, 6, 8, 9, 11, 13, 14, 15];
	///
	/// let evens: Vec<i32> = numbers
	/// 	.iter_take()
	/// 	.filter(|i| &**i % 2 == 0)
	/// 	.map(Take::take)
	/// 	.collect();
	///
	/// let odds = numbers;
	///
	/// assert_eq!(evens, vec![2, 4, 6, 8, 14]);
	/// assert_eq!(odds, vec![1, 3, 5, 9, 11, 13, 15]);
	/// ```
	///
	/// # Drain
	///
	/// ```
	/// # use bad_take::*;
	/// let mut result_src = vec!(0, 1, 2, 3, 4, 5, 6, 7);
	/// let mut expected_src = result_src.clone();
	///
	/// let mut result_dst = Vec::new();
	/// let mut expected_dst = Vec::new();
	///
	/// expected_src
	/// 	.drain(3..5)
	/// 	.for_each(|removed| expected_dst.push(removed));
	///
	/// result_src
	/// 	.iter_take()
	/// 	.skip(3).take(2)
	/// 	.for_each(|removed| result_dst.push(Take::take(removed)));
	///
	/// assert_eq!(expected_src, result_src);
	/// assert_eq!(result_dst, result_dst);
	/// ```
	///
	/// # Drain filter
	///
	/// ```
	/// #![feature(drain_filter)]
	///
	/// # use bad_take::*;
	/// let mut result_src = vec!(0, 1, 2, 3, 4, 5, 6, 7);
	/// let mut expected_src = result_src.clone();
	///
	/// let mut result_dst = Vec::new();
	/// let mut expected_dst = Vec::new();
	///
	/// expected_src
	/// 	.drain_filter(|&mut i| i % 3 == 0)
	/// 	.for_each(|removed| expected_dst.push(removed));
	///
	/// result_src
	/// 	.iter_take()
	/// 	.filter(|i| **i % 3 == 0)
	/// 	.for_each(|removed| result_dst.push(Take::take(removed)));
	///
	/// assert_eq!(expected_src, result_src);
	/// assert_eq!(result_src, result_src);
	/// ```
	///
	/// # Panics
	///
	/// See [`IterTake::next`]: Panics
	fn iter_take(&mut self) -> IterTake<Self::Item>;
}

impl<T> IterTakeExt for Vec<T> {
	type Item = T;

	fn iter_take(&mut self) -> IterTake<Self::Item> {
		IterTake::new(self)
	}
}

pub struct IterTake<'a, T> {
	inner: NonNull<Vec<T>>,
	index: usize,

	/// NOTE: this is needed to stop multiple `Take`s from aliasing
	panic: bool,

	_item: PhantomData<&'a mut T>,
}

impl<'a, T> IterTake<'a, T> {
	pub(crate) fn new(inner: &'a mut Vec<T>) -> Self {
		Self {
			inner: NonNull::from(inner),
			index: 0,
			panic: false,
			_item: PhantomData,
		}
	}
}

impl<'a, T: 'a> Iterator for IterTake<'a, T> {
	type Item = Take<'a, T>;

	/// # Panics
	///
	/// This function panics if it is called before the previous `Take` is either [`drop`]ped or consumed with [`Take::take`]
	///
	/// ```should_panic
	/// # use bad_take::*;
	/// let mut v = vec![0, 1, 2, 3, 4];
	///
	/// let mut i = v.iter_take();
	///
	/// let a = i.next().unwrap();
	/// let b = i.next().unwrap();
	///
	/// assert_eq!(&*a, &*b);
	/// ```
	fn next(&mut self) -> Option<Self::Item> {
		if self.panic {
			panic!("called `IterTake::next` without destroying `Take` first")
		}

		if self.index == unsafe { self.inner.as_ref().len() } {
			return None
		}

		self.panic = true;

		Some(Take {
			index: self.index,
			parent: NonNull::from(self),
		})
	}
}

pub struct Take<'a, T> {
	parent: NonNull<IterTake<'a, T>>,
	index: usize,
}

impl<'a, T> Take<'a, T> {
	/// Removes and returns the current element from the vector while preserving the order
	pub fn take(self) -> T {
		unsafe {
			let mut this = std::mem::ManuallyDrop::new(self);

			let parent = this.parent.as_mut();

			debug_assert!(parent.panic, "a `Take` existed without the `panic` flag being set");

			parent.panic = false;
			parent.inner.as_mut().remove(this.index)
		}
	}
	/// Removes and returns the current element from the vector
	///
	/// The order of the rest of the elements is may change
	pub fn take_unstable(self) -> T {
		unsafe {
			let mut this = std::mem::ManuallyDrop::new(self);

			let parent = this.parent.as_mut();

			debug_assert!(parent.panic, "a `Take` existed without the `panic` flag being set");

			parent.panic = false;
			parent.inner.as_mut().swap_remove(this.index)
		}
	}
}

impl<'a, T> Drop for Take<'a, T> {
	fn drop(&mut self) {
		unsafe {
			let parent = self.parent.as_mut();

			debug_assert!(parent.panic, "a `Take` existed without the `panic` flag being set");

			parent.panic = false;
			parent.index += 1;
		}
	}
}

impl<'a, T> Deref for Take<'a, T> {
	type Target = T;

	fn deref(&self) -> &Self::Target {
		unsafe { self.parent.as_ref().inner.as_ref().get_unchecked(self.index) }
	}
}

impl<'a, T> DerefMut for Take<'a, T> {
	fn deref_mut(&mut self) -> &mut Self::Target {
		unsafe {
			let parent = self.parent.as_mut();

			debug_assert!(parent.panic, "a `Take` existed without the `panic` flag being set");

			parent.inner.as_mut().get_unchecked_mut(self.index)
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn iter_mut_parity() {
		let mut result = vec![0, 1, 2, 3, 4, 5, 6, 7];
		let expected = result.clone();

		result.iter_take().for_each(drop);

		assert_eq!(result, expected)
	}

	#[test]
	fn take_all_takes_all() {
		let mut src = vec![0, 1, 2, 3, 4];
		let expected = src.clone();

		let result: Vec<i32> = src.iter_take().map(Take::take).collect();

		assert!(src.is_empty());
		assert_eq!(result, expected);
	}

	#[test]
	#[should_panic = "called `IterTake::next` without destroying `Take` first"]
	fn no_trivial_mut_alias() {
		let mut v = vec![0, 1, 2, 3, 4];
		let mut i = v.iter_take();

		let a = i.next().unwrap();
		let b = i.next().unwrap();

		assert_eq!(&*a, &*b);
	}
}
