// "Tifflin" Kernel
// - By John Hodge (thePowersGang)
//
// Core/lib/string.rs
//! Dynamically-allocated string type
//!
//! Acts every similarly to the rust std's String type.
use _common::*;
use core::ops;

/// String type
#[derive(Clone)]
pub struct String(Vec<u8>);

/// String backed to a statically-allocated buffer
pub struct FixedString<Buf: AsMut<[u8]>+AsRef<[u8]>>
{
	data: Buf,
	len: usize,
}

impl String
{
	/// Create a new empty string (with no allocation)
	pub fn new() -> String {
		String(Vec::new())
	}
	/// Create a pre-allocated string capable of holding `cap` bytes
	pub fn with_capacity(cap: usize) -> String {
		String(Vec::with_capacity(cap))
	}
	/// Create a string from a string slice
	pub fn from_str(string: &str) -> String {
		let mut v = Vec::new();
		v.push_all(string.as_bytes());
		String(v)
	}
	/// Create a string from a `fmt::Arguments` instance (used by `format!`)
	pub fn from_args(args: ::core::fmt::Arguments) -> String {
		use core::fmt::Write;
		let mut ret = String::new();
		let _ = write!(&mut ret, "{}", args);
		ret
	}
	
	/// Append `s` to the string
	pub fn push_str(&mut self, s: &str)
	{
		self.0.push_all(s.as_bytes());
	}
	
	/// Return the string as a &str
	fn as_slice(&self) -> &str {
		let bytes: &[u8] = self.0.as_ref();
		unsafe { ::core::mem::transmute( bytes ) }
	}
}

impl ::core::default::Default for String
{
	fn default() -> String { String::new() }
}

impl ::core::fmt::Write for String
{
	fn write_str(&mut self, s: &str) -> ::core::fmt::Result
	{
		self.push_str(s);
		Ok( () )
	}
}

impl ::core::ops::Deref for String
{
	type Target = str;
	fn deref(&self) -> &str
	{
		self.as_slice()
	}
}

impl ::core::fmt::Display for String
{
	fn fmt(&self, f: &mut ::core::fmt::Formatter) -> ::core::fmt::Result
	{
		::core::fmt::Display::fmt(self.as_slice(), f)
	}
}

impl<'a> From<&'a str> for String
{
	fn from(v: &str) -> String {
		String::from_str(v)
	}
}


impl<B: AsMut<[u8]>+AsRef<[u8]>> FixedString<B>
{
	/// Create a new fixed-capacity string using the provided buffer
	pub fn new(backing: B) -> FixedString<B> {
		assert!(backing.as_ref().len() > 0);
		FixedString {
			data: backing,
			len: 0,
		}
	}
	fn push_char(&mut self, c: char) {
		match c.encode_utf8(&mut self.data.as_mut()[self.len..])
		{
		Some(l) => self.len += l,
		None => todo!("Freeze string once allocation exceeded"),
		}
	}
	/// Append a slice
	pub fn push_str(&mut self, s: &str) {
		self.extend( s.chars() );
	}
}
impl<B: AsMut<[u8]>+AsRef<[u8]>> ::core::iter::Extend<char> for FixedString<B>
{
	fn extend<T>(&mut self, iterable: T)
	where
		T: ::core::iter::IntoIterator<Item=char>
	{
		for c in iterable {
			self.push_char(c);
		}
	}
}
impl<B: AsMut<[u8]>+AsRef<[u8]>> ops::Deref for FixedString<B>
{
	type Target = str;
	fn deref(&self) -> &str {
		let bytes = &self.data.as_ref()[..self.len];
		unsafe { ::core::mem::transmute(bytes) }
	}
}

/// Construct a `String` using a format string and arguments
#[macro_export]
macro_rules! format {
	($($arg:tt)*) => ($crate::lib::string::String::from_args(format_args!($($arg)*)))
}

// vim: ft=rust
