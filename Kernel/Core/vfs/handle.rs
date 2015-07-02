// "Tifflin" Kernel
// - By John Hodge (thePowersGang)
//
// Core/vfs/handle.rs
//! Opened file interface
use prelude::*;
use super::node::{CacheHandle,NodeType};
use lib::byte_str::ByteString;
use super::Path;

#[derive(Debug)]
/// Open without caring what the file type is (e.g. enumeration)
pub struct Any {
	node: CacheHandle,
}
#[derive(Debug)]
/// Normal file
pub struct File {
	node: CacheHandle,
	mode: FileOpenMode,
}
#[derive(Debug)]
/// Directory (for enumeration)
pub struct Dir {
	node: CacheHandle,
}
#[derive(Debug)]
/// Symbolic link (allows reading the link contents)
pub struct Symlink {
	node: CacheHandle,
}
#[derive(Debug)]
/// Special file (?API exposed)
pub struct Special {
	node: CacheHandle,
}

#[derive(Debug)]
pub enum FileOpenMode
{
	/// Shared read-only, multiple readers but no writers visible
	///
	/// When opened in this manner, the file contents cannot change, but it might extend
	SharedRO,
	/// Open for execution (multiple readers)
	///
	/// No file changes visible to handles, must be an executable file
	Execute,
	/// Eclusive read-write, denies any other opens while held (except Append)
	///
	/// No changes to the file will be visible to the user (as the file is locked)
	ExclRW,
	/// Unique read-write, does Copy-on-write to create a new file
	///
	/// No changes to the file will be visible to the user (as it has its own copy)
	UniqueRW,
	/// Append only (allows multiple readers/writers)
	///
	/// Cannot read, all writes go to the end of the file (a write call is atomic)
	Append,
	/// Unsynchronised read-write
	///
	/// No synchronisation at all, fails if any other open type is active.
	Unsynch,
}

#[derive(Debug)]
pub enum MemoryMapMode
{
	/// Read-only mapping of a file
	ReadOnly,
	/// Executable mapping of a file
	Execute,
	/// Copy-on-write (used for executable files)
	COW,
	/// Allows writing to the backing file
	WriteBack,
}

impl Any
{
	/// Open the specified path (not caring what the actual type is)
	pub fn open(path: &Path) -> super::Result<Any> {
		let node = try!(CacheHandle::from_path(path));
		Ok(Any { node: node })
	}
	
	/// Upgrade the handle to a directory handle
	pub fn to_dir(self) -> super::Result<Dir> {
		if self.node.is_dir() {
			Ok(Dir { node: self.node })
		}
		else {
			Err(super::Error::TypeMismatch)
		}
	}
}

pub struct MemoryMapHandle<'a>
{
	handle: &'a File,
	base: *mut (),
	len: usize,
}

impl File
{
	/// Open the specified path as a file
	pub fn open(path: &Path, mode: FileOpenMode) -> super::Result<File> {
		let node = try!(CacheHandle::from_path(path));
		if !node.is_file() {
			return Err(super::Error::TypeMismatch);
		}
		match mode
		{
		// TODO: Mark file as shared
		// TODO: Check permissions (must be readable in current context)
		FileOpenMode::SharedRO => {},
		// TODO: Mark file as shared
		// TODO: Check permissions (must be executable in current context)
		FileOpenMode::Execute => {},
		_ => todo!("Acquire lock depending on mode({:?})", mode),
		}
		Ok(File { node: node, mode: mode })
	}
	
	pub fn size(&self) -> u64 {
		self.node.get_valid_size()
	}

	/// Read data from the file at the specified offset
	///
	/// Returns the number of read bytes (which might be less than the size of the input
	/// slice).
	pub fn read(&self, ofs: u64, dst: &mut [u8]) -> super::Result<usize> {
		assert!(self.node.is_file());
		self.node.read(ofs, dst)
	}

	
	/// Map a file into the address space
	pub fn memory_map(&self, address: usize, ofs: u64, size: usize, mode: MemoryMapMode) -> super::Result<MemoryMapHandle> {
		// - Check that this file is opened in a sufficent mode to allow this form of mapping
		match mode
		{
		// Read only - Pretty much anything
		MemoryMapMode::ReadOnly => match self.mode
			{
			FileOpenMode::Execute => {},
			FileOpenMode::SharedRO => {},
			//FileOpenMode::ExclRW => /* NOTE: Needs extra checks to ensure that aliasing does not occur */
			//FileOpenMode::UniqueRW => /* NOTE: Needs extra checks to ensure that aliasing does not occur */
			_ => return Err(super::Error::PermissionDenied),
			},
		// Executable - Execute mode only
		MemoryMapMode::Execute => match self.mode
			{
			FileOpenMode::Execute => {},
			_ => return Err(super::Error::PermissionDenied),
			},
		// COW - Execute mode only
		// - TODO: Could allow COW of readonly files? (as soon as it's written, the page is detached from the file)
		MemoryMapMode::COW => match self.mode
			{
			FileOpenMode::Execute => {},
			//FileOpenMode::SharedRO => {},
			_ => return Err(super::Error::PermissionDenied),
			},
		// Writeback - Requires exclusive access to the file (or a copy)
		MemoryMapMode::WriteBack => match self.mode
			{
			//FileOpenMode::ExclRW => /* NOTE: Needs extra checks to ensure that aliasing does not occur */
			//FileOpenMode::UniqueRW => /* NOTE: Needs extra checks to ensure that aliasing does not occur */
			_ => return Err(super::Error::PermissionDenied),
			},
		}
		
		// TODO: Handle unaligned mappings somehow
		// - Depends on several qirks:
		//  > Unaligned address could write to an existing page (converting it to a private) - But how would that interact with existing mappings?
		//  > Unaligned sizes would usually cause a new anon mapping, but if its unaligned becuase of EOF, it should just be COW as usual
		assert!(address % ::PAGE_SIZE == 0, "TODO: Unaligned memory_map (address={})", address);
		assert!(size % ::PAGE_SIZE == 0, "TODO: Unaligned memory_map (size={})", size);
		if address % ::PAGE_SIZE != (ofs % ::PAGE_SIZE as u64) as usize {
			return Err( super::Error::Unknown("memory_map alignment mismatch") );
		}
		// - Limit checking (ofs + size must be within size of the file)
		// TODO: Limit checking
		// - Reserve the region to be mapped (reserve sticks a zero page in)
		let page_count = size / ::PAGE_SIZE;
		let mut resv = match ::memory::virt::reserve(address as *mut (), page_count)
			{
			Ok(v) => v,
			Err(_) => return Err( super::Error::Locked ),
			};
		// - Obtain handles to each cached page, and map into the reservation
		for i in 0 .. page_count {
			let page = ofs / ::PAGE_SIZE as u64 + i as u64;
			// 1. Search the node for this particular page
			//let lh = self.page_cache.read();
			//  - If found, map over region
			// 2. Drop lock, read data from file, and try again
			//drop(lh)
			try!( self.node.read(page * ::PAGE_SIZE as u64, resv.get_mut_page(i)) );
			// 3. Acquire write on lock, and attempt to insert a handle to this page
			//let lh = self.page_cache.write();
			//match lh.try_insert(pag, self.get_page_handle(i))
			//{
			//Ok(_) => {},	// Inserted correctly
			//Err(h) => {	// Another handle made a page for this first
			//	resv.map_at(i, h);	// - Map over our original attempt
			//	},
			//}
		}
		resv.finalise( match mode
			{
			MemoryMapMode::ReadOnly  => ::memory::virt::ProtectionMode::UserRO,
			MemoryMapMode::Execute   => ::memory::virt::ProtectionMode::UserRX,
			MemoryMapMode::COW       => ::memory::virt::ProtectionMode::UserCOW,
			MemoryMapMode::WriteBack => ::memory::virt::ProtectionMode::UserRW,
			})
			.unwrap();
		Ok(MemoryMapHandle {
			handle: self,
			base: address as *mut (),
			len: page_count * ::PAGE_SIZE,
			})
	}
}
impl ::core::ops::Drop for File
{
	fn drop(&mut self) {
		match self.mode
		{
		FileOpenMode::SharedRO => {},
		_ => todo!("File::drop() - mode={:?}", self.mode),
		}
		// TODO: For files, we need to release the lock
	}
}

impl<'a> Drop for MemoryMapHandle<'a>
{
	fn drop(&mut self)
	{
		todo!("MemoryMapHandle::drop");
	}
}

impl Dir
{
	/// Open a provided path as a directory
	pub fn open(path: &Path) -> super::Result<Dir> {
		try!(Any::open(path)).to_dir()
	}
	
	pub fn iter(&self) -> DirIter {
		DirIter {
			handle: self,
			ents: [
				Default::default(), Default::default(),
				Default::default(), Default::default(),
				],
			pos: 0,
			ofs: 0,
			count: 0,
		}
	}
	
	/// Create a new directory
	pub fn mkdir(&self, name: &str) -> super::Result<Dir> {
		let node = try!(self.node.create(name.as_ref(), NodeType::Dir));
		assert!(node.is_dir());
		Ok( Dir { node: node } )
	}
	/// Create a new symbolic link
	pub fn symlink(&self, name: &str, target: &Path) -> super::Result<()> {
		try!(self.node.create(name.as_ref(), NodeType::Symlink(target)));
		Ok( () )
	}
}

pub struct DirIter<'a> {
	handle: &'a Dir,
	count: usize,
	ofs: usize,
	pos: usize,
	ents: [(super::node::InodeId,ByteString); 4],
}
impl<'a> ::core::iter::Iterator for DirIter<'a> {
	type Item = ByteString;
	fn next(&mut self) -> Option<ByteString> {
		if self.ofs == self.count {
			match self.handle.node.read_dir(self.pos, &mut self.ents)
			{
			Err(e) => {
				log_warning!("Error while iterating dir: {:?}", e);
				return None;
				},
			Ok((next,count)) => {
				self.pos = next;
				self.count = count;
				},
			}
			if self.count == 0 {
				return None;
			}
			self.ofs = 1;
		}
		else {
			self.ofs += 1;
		}
		Some( ::core::mem::replace(&mut self.ents[self.ofs-1].1, ByteString::new()) )
	}
}

