// "Tifflin" Kernel - ISO9660 Filesystem Driver
// - By John Hodge (thePowersGang)
//
// Modules/fs_iso9660/lib.rs
#![feature(no_std,core,linkage)]
#![no_std]
#[macro_use] extern crate core;
#[macro_use] extern crate kernel;
use kernel::prelude::*;

use kernel::vfs::{self, mount, node};
use kernel::metadevs::storage::{self,VolumeHandle,SizePrinter};
use kernel::lib::mem::aref::{ArefInner,ArefBorrow};
use kernel::lib::byteorder::{ByteOrder,LittleEndian};
use kernel::lib::byte_str::{ByteStr,ByteString};

module_define!{FS_ISO9660, [VFS], init}

//mod ondisk;

struct Driver;
static S_DRIVER: Driver = Driver;

struct Instance(ArefInner<InstanceInner>);
impl ::core::ops::Deref for Instance {
	type Target = InstanceInner;
	fn deref(&self) -> &InstanceInner { &self.0 }
}

struct InstanceInner
{
	vh: VolumeHandle,
	lb_size: usize,
	root_lba: u32,
	root_size: u32,
}

fn init()
{
	let h = mount::DriverRegistration::new("iso9660", &S_DRIVER);
	// TODO: Remember the registration for unloading
	::core::mem::forget(h);
}

impl mount::Driver for Driver
{
	fn detect(&self, vol: &VolumeHandle) -> vfs::Result<usize> {
		let bs = vol.block_size() as u64;
		let blk = {
			let mut block: Vec<_> = (0 .. bs).map(|_|0).collect();
			try!(vol.read_blocks(32*1024 / bs, &mut block));
			block
			};
		if &blk[1..6] == b"CD001" {
			Ok(1)
		}
		else {
			Ok(0)
		}
	}
	fn mount(&self, vol: VolumeHandle) -> vfs::Result<Box<mount::Filesystem>> {
		if 2048 % vol.block_size() != 0 {
			return Err( vfs::Error::Unknown("Can't mount ISO9660 with sector size not a factor of 2048"/*, vol.block_size()*/) );
		}
		let scale = 2048 / vol.block_size();
		
		let mut block: Vec<u8> = (0 .. 2048).map(|_|0).collect();
		for sector in (16 .. )
		{
			try!(vol.read_blocks((sector*scale) as u64, &mut block));
			if &block[1..6] != b"CD001" {
				return Err( vfs::Error::Unknown("Invalid volume descriptor present") );
			}
			else if block[0] == 255 {
				return Err( vfs::Error::Unknown("Can't find ISO9660 primary volume descriptor") );
			}
			else if block[0] == 0x01 {
				// Found it!
				break ;
			}
			else {
				// Try the next one
			}
		}
		::kernel::logging::hex_dump("ISO966 PVD", &block);
		
		// Obtain the logical block size (different from medium sector size)
		let lb_size = LittleEndian::read_u16(&block[128..]);
		// Extract the root directory entry
		// - We want the LBA and byte length
		let root_lba  = LittleEndian::read_u32(&block[156+ 2..]);
		let root_size = LittleEndian::read_u32(&block[156+10..]);
		
		log_debug!("lb_size = {}, root = {:#x} + {:#x} bytes", lb_size, root_lba, root_size);
		
		Ok( Box::new( Instance(
			unsafe { ArefInner::new( InstanceInner {
				vh: vol,
				lb_size: lb_size as usize,
				root_lba: root_lba,
				root_size: root_size,
				} ) }
			) ) )
	}
}

impl mount::Filesystem for Instance
{
	fn root_inode(&self) -> node::InodeId {
		self.root_lba as node::InodeId
	}
	fn get_node_by_inode(&self, id: node::InodeId) -> Option<node::Node> {
		if id == self.root_lba as node::InodeId {
			Some(Dir::new_node(self.0.borrow(), self.root_lba, self.root_size) )
		}
		else {
			// Look up (or read) parent directory to obtain the 
			todo!("Instance::get_node_by_inode({:#x})", id);
		}
	}
}
impl InstanceInner
{
	fn read_sector(&self, sector: u32) -> Result<Vec<u8>, storage::IoError> {
		// TODO: Cache sector data
		// - I would like to keep Rc<[u8]> in a VecMap or similar, keep four or so around as a LRU cache.
		let mut ret: Vec<u8> = ::core::iter::repeat(0).take(self.lb_size).collect();
		try!( self.vh.read_blocks( (sector as usize * self.lb_size / self.vh.block_size()) as u64, &mut ret) );
		Ok( ret )
	}
}

struct Dir
{
	fs: ArefBorrow<InstanceInner>,
	first_lba: u32,
	size: u32,
}
impl Dir
{
	fn new_node(fs: ArefBorrow<InstanceInner>, first_lba: u32, size: u32) -> node::Node {
		node::Node::Dir( Box::new( Dir {
			fs: fs,
			first_lba: first_lba,
			size: size,
			} ) )
	}
}
impl node::NodeBase for Dir
{
	fn get_id(&self) -> node::InodeId { todo!("Dir::get_id") }
}
impl node::Dir for Dir
{
	fn lookup(&self, name: &ByteStr) -> node::Result<node::InodeId> {
		todo!("Dir::lookup({:?})", name)
	}
	fn read(&self, ofs: usize, items: &mut [(node::InodeId,ByteString)]) -> node::Result<(usize,usize)> {
		let (end_sect,end_ofs) = (self.size as usize / self.fs.lb_size, self.size as usize % self.fs.lb_size);
		let mut sector = ofs / self.fs.lb_size;
		let mut ofs = ofs % self.fs.lb_size;
		
		let mut count = 0;
		
		let mut data = try!(self.fs.read_sector(self.first_lba + sector as u32));
		while !(sector == end_sect && ofs >= end_ofs)
		{
			let len = data[ofs] as usize;
			if len == 0 {
				ofs += 1;
			}
			else {
				let ent = &data[ofs .. ofs + len];
				ofs += len;
				
				let namelen = ent[32] as usize;
				let name = &ent[33 .. 33 + namelen];
				let start = LittleEndian::read_u32(&ent[2..]);
				
				items[count] = (start as node::InodeId, ByteString::from(name));
				count += 1;
				if count == items.len() {
					break;
				}
			}
			if ofs >= self.fs.lb_size {
				sector += 1;
				ofs = 0;
				data = try!(self.fs.read_sector(self.first_lba + sector as u32));
			}
		}
		
		Ok( (sector*self.fs.lb_size + ofs, count) )
	}
	
	fn create(&self, name: &ByteStr, nodetype: node::NodeType) -> node::Result<node::InodeId> {
		Err( node::IoError::ReadOnly )
	}
	fn link(&self, name: &ByteStr, inode: node::InodeId) -> node::Result<()> {
		Err( node::IoError::ReadOnly )
	}
	fn unlink(&self, name: &ByteStr) -> node::Result<()> {
		Err( node::IoError::ReadOnly )
	}
}
