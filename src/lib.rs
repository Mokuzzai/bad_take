#![cfg_attr(test, feature(drain_filter))]

use std::marker::PhantomData;
use std::ops::Deref;
use std::ops::DerefMut;
use std::ptr::NonNull;

pub trait IterTakeExt {
	type Item;

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
	_item: PhantomData<&'a mut T>,
}

impl<'a, T> IterTake<'a, T> {
	pub(crate) fn new(inner: &'a mut Vec<T>) -> Self {
		Self {
			inner: NonNull::from(inner),
			index: 0,
			_item: PhantomData,
		}
	}
}

impl<'a, T: 'a> Iterator for IterTake<'a, T> {
	type Item = Take<'a, T>;

	fn next(&mut self) -> Option<Self::Item> {
		if self.index == unsafe { self.inner.as_ref().len() } {
			return None
		}

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
	pub fn take(self) -> T {
		unsafe {
			let mut this = std::mem::ManuallyDrop::new(self);

			let parent = this.parent.as_mut();

			parent.inner.as_mut().remove(this.index)
		}
	}
}

impl<'a, T> Drop for Take<'a, T> {
	fn drop(&mut self) {
		unsafe {
			self.parent.as_mut().index += 1;
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
		unsafe { self.parent.as_mut().inner.as_mut().get_unchecked_mut(self.index) }
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

		assert_eq!(result, expected);
	}

	#[test]
	fn drain_parity() {
		let mut result_src = vec!(0, 1, 2, 3, 4, 5, 6, 7);
		let mut expected_src = result_src.clone();

		let mut result_dst = Vec::new();
		let mut expected_dst = Vec::new();

		expected_src.drain(3..3+2).for_each(|removed| expected_dst.push(removed));
		result_src.iter_take().skip(3).take(2).for_each(|removed| result_dst.push(Take::take(removed)));

		assert_eq!(expected_src, result_src);
		assert_eq!(result_src, result_src);
	}

	#[test]
	fn drain_filter_parity() {
		let mut result_src = vec!(0, 1, 2, 3, 4, 5, 6, 7);
		let mut expected_src = result_src.clone();

		let mut result_dst = Vec::new();
		let mut expected_dst = Vec::new();

		expected_src.drain_filter(|&mut i| i % 3 == 0).for_each(|removed| expected_dst.push(removed));
		result_src.iter_take().filter(|i| **i % 3 == 0).for_each(|removed| result_dst.push(Take::take(removed)));

		assert_eq!(expected_src, result_src);
		assert_eq!(result_src, result_src);
	}

	#[test]
	/// a simple test to prove that our api is unsound
	// FIXME: `Take` could be a linear type?
	fn is_unsound() {
		let mut v = vec![0, 1, 2, 3, 4];

		let mut i = v.iter_take();

		let a = i.next().unwrap();
		// FIXME: we must call `drop` or `Take::take` on `a` before we call next;
		// FIXME: add flag;
		let b = i.next().unwrap();

		assert_eq!(&*a, &*b);
	}
}
