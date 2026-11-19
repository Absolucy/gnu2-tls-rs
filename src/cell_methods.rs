// SPDX-License-Identifier: MPL-2.0
//! std's `Cell` and `RefCell` shortcuts on `LocalKey`, same behavior as std.

use crate::LocalKey;
use core::cell::{Cell, RefCell};

impl<T: 'static> LocalKey<Cell<T>> {
	/// Sets the value. On a thread that hasn't touched the key yet, this skips
	/// the declared initializer.
	#[track_caller]
	pub fn set(&'static self, value: T) {
		self.initialize_with(Cell::new(value), |unused, cell| {
			if let Some(value) = unused {
				cell.set(value.into_inner());
			}
		});
	}

	/// Returns a copy of the value.
	#[track_caller]
	pub fn get(&'static self) -> T
	where
		T: Copy,
	{
		self.with(Cell::get)
	}

	/// Takes the value, leaving `Default::default()` behind.
	#[track_caller]
	pub fn take(&'static self) -> T
	where
		T: Default,
	{
		self.with(Cell::take)
	}

	/// Replaces the value, returning the old one.
	#[track_caller]
	pub fn replace(&'static self, value: T) -> T {
		self.with(|cell| cell.replace(value))
	}

	/// Updates the value with `f`.
	#[track_caller]
	pub fn update(&'static self, f: impl FnOnce(T) -> T)
	where
		T: Copy,
	{
		self.with(|cell| cell.update(f));
	}
}

impl<T: 'static> LocalKey<RefCell<T>> {
	/// Calls `f` with a shared borrow of the value.
	#[track_caller]
	pub fn with_borrow<F, R>(&'static self, f: F) -> R
	where
		F: FnOnce(&T) -> R,
	{
		self.with(|cell| f(&cell.borrow()))
	}

	/// Calls `f` with a mutable borrow of the value.
	#[track_caller]
	pub fn with_borrow_mut<F, R>(&'static self, f: F) -> R
	where
		F: FnOnce(&mut T) -> R,
	{
		self.with(|cell| f(&mut cell.borrow_mut()))
	}

	/// Sets the value. On a thread that hasn't touched the key yet, this skips
	/// the declared initializer.
	#[track_caller]
	pub fn set(&'static self, value: T) {
		self.initialize_with(RefCell::new(value), |unused, cell| {
			if let Some(value) = unused {
				*cell.borrow_mut() = value.into_inner();
			}
		});
	}

	/// Takes the value, leaving `Default::default()` behind.
	#[track_caller]
	pub fn take(&'static self) -> T
	where
		T: Default,
	{
		self.with(RefCell::take)
	}

	/// Replaces the value, returning the old one.
	#[track_caller]
	pub fn replace(&'static self, value: T) -> T {
		self.with(|cell| cell.replace(value))
	}
}
