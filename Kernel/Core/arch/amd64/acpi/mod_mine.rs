// "Tifflin" Kernel
// - By John Hodge (thePowersGang)
//
// arch/amd64/acpi/mod_mine.rs
//! ACPI Component Architecture binding
use _common::*;
use core::fmt;
use core::str::from_utf8;

use self::TLSDT::{TopRSDT,TopXSDT};
use super::{SDT,SDTHeader};
pub use super::GAS;

mod fadt;
mod aml;

/// A handle to a SDT
pub struct SDTHandle<T:'static>
{
	maphandle: ::memory::virt::AllocHandle,
	ofs: usize,
	_type: ::core::marker::PhantomData<T>,
}

struct ACPI
{
	top_sdt: TLSDT,
	names: Vec<[u8; 4]>,
}

enum TLSDT
{
	TopRSDT(&'static SDT<RSDT>),
	TopXSDT(&'static SDT<XSDT>),
}

#[repr(C,packed)]
struct RSDP
{
	signature: [u8; 8],
	checksum: u8,
	oemid: [u8; 6],
	revision: u8,
	rsdt_address: u32,
}
#[repr(C,packed)]
struct RSDPv2
{
	v1: RSDP,
	// Version 2.0
	length: u32,
	xsdt_address: u64,
	ext_checksum: u8,
	_resvd1: [u8; 3],
}

#[repr(C)]
struct RSDT
{
	pointers: u32,
}

#[repr(C)]
struct XSDT
{
	pointers: u64,	// Rust doesn't support arbitary length arrays
}

static S_ACPI_STATE: ::lib::LazyStatic<ACPI> = lazystatic_init!();

/// ACPI module init - Locate the [RX]SDT
pub fn init()
{
	let rsdp = match get_rsdp() {
		Some(x) => x,
		None => {
			log_notice!("Cannot find RSDP");
			return;
			}
		};
	log_debug!("RSDP = {{ oemid = {:?}, revision = {:#x}, rsdt_address = {:#x} }}",
		::core::str::from_utf8(&rsdp.oemid), rsdp.revision, rsdp.rsdt_address);
	
	// Determine the top-level SDT type
	let tl = if rsdp.revision == 0 {
			TopRSDT( SDTHandle::<RSDT>::new( rsdp.rsdt_address as u64 ).make_static() )
		} else {
			let v2: &RSDPv2 = unsafe { ::core::mem::transmute(rsdp) };
			if sum_struct(v2) != 0 {
				// oh
				panic!("RSDPv2 checksum failed");
			}
			TopXSDT( SDTHandle::<XSDT>::new( v2.xsdt_address ).make_static() )
		};
	log_debug!("*SDT = {{ signature = {}, oemid = '{}' }}", tl.signature(), tl.oemid());
	
	// Obtain list of SDTs (signatures only)
	let names = (0 .. tl.len()).map( |i| tl.get::<SDTHeader>(i).raw_signature() ).collect();
	
	unsafe {
		S_ACPI_STATE.prep(|| ACPI { top_sdt: tl, names: names, });
	}
	
	
	// Poke sub-enumerators
	if false {
		fadt::parse_fadt();
	}
}

/// Find all SDTs with a given signature
pub fn find_table<T:'static>(req_name: &str, mut idx: usize) -> Option<SDTHandle<T>>
{
	log_debug!("find('{}',{})", req_name, idx);
	assert_eq!(req_name.len(), 4);
	for (i,ent_name) in S_ACPI_STATE.names.iter().enumerate()
	{
		if &ent_name[..] != req_name.as_bytes()
		{
		}
		else if idx > 0
		{
			idx -= 1;
		}
		else
		{
			let table = S_ACPI_STATE.top_sdt.get::<T>(i);
			if (*table).validate() == false {
				log_error!("ACPI ent #{} failed checksum", i);
			}
			return Some( table );
		}
	}
	None
}
pub fn count_tables(req_name: &str) -> usize {
	assert_eq!(req_name.len(), 4);
	S_ACPI_STATE.names.iter().filter(|n| &n[..] == req_name.as_bytes()).count()
}

/// Obtain a reference to the RSDP (will be in the identity mapping area)
fn get_rsdp() -> Option<&'static RSDP>
{
	unsafe {
	let ebda_ver = locate_rsdp((::arch::memory::addresses::IDENT_START + 0x9FC00) as *const u8, 0x400);
	if !ebda_ver.is_null() {
		return ebda_ver.as_ref();
	}
	let bios_ver = locate_rsdp((::arch::memory::addresses::IDENT_START + 0xE0000) as *const u8, 0x20000);
	if !bios_ver.is_null() {
		return bios_ver.as_ref();
	}
	}
	return None;
}
/// Search a section of memory for the RSDP
unsafe fn locate_rsdp(base: *const u8, size: usize) -> *const RSDP
{
	//for ofs in (0 .. size).step_by(16)
	for i in (0 .. size)
	{
		let ofs = i * 16;
		let sig = base.offset(ofs as isize) as *const [u8; 8];
		if &*sig == b"RSD PTR "
		{
			let ret = sig as *const RSDP;
			if sum_struct(&*ret) == 0
			{
				return ret;
			}
		}
	}
	::core::ptr::null()
}

/// Caclulate the byte sum of a structure
fn sum_struct<T>(s: &T) -> u8
{
	unsafe {
		let ptr = s as *const T as *const u8;
		let vals = ::core::slice::from_raw_parts(ptr, ::core::mem::size_of::<T>());
		vals.iter().fold(0, |a,&b| a+b)
	}
}

impl TLSDT
{
	fn _header<'self_>(&'self_ self) -> &'self_ SDTHeader {
		match self {
		&TopRSDT(sdt) => &(*sdt).header,
		&TopXSDT(sdt) => &(*sdt).header,
		}
	}
	fn _getaddr(&self, idx: usize) -> u64 {
		match self {
		&TopRSDT(sdt) => (*sdt).getptr(idx),
		&TopXSDT(sdt) => (*sdt).getptr(idx),
		}
	}
	
	fn len(&self) -> usize {
		(self._header().length as usize - ::core::mem::size_of::<SDTHeader>()) / match self {
			&TopRSDT(_) => 4,
			&TopXSDT(_) => 8,
			}
	}
	
	fn signature<'self_>(&'self_ self) -> &'self_ str {
		from_utf8(&self._header().signature).unwrap()
	}
	fn oemid<'self_>(&'self_ self) -> &'self_ str {
		from_utf8(&self._header().oemid).unwrap()
	}
	fn get<T>(&self, idx: usize) -> SDTHandle<T> {
		SDTHandle::<T>::new(self._getaddr(idx))
	}
}
trait RSDTTrait
{
	fn getptr(&self, idx: usize) -> u64;
}

impl RSDTTrait for SDT<RSDT>
{
	fn getptr(&self, idx: usize) -> u64
	{
		let ptrs = &(self.data.pointers) as *const u32;
		assert!( !ptrs.is_null() );
		unsafe {
			*ptrs.offset(idx as isize) as u64
		}
	}
}
impl RSDTTrait for SDT<XSDT>
{
	fn getptr(&self, idx: usize) -> u64
	{
		let ptrs = &(self.data.pointers) as *const u64;
		assert!( !ptrs.is_null() );
		unsafe {
			*ptrs.offset(idx as isize)
		}
	}
}

impl ::core::fmt::Debug for SDTHeader
{
	fn fmt(&self, f: &mut ::core::fmt::Formatter) -> ::core::fmt::Result
	{
		write!(f, "SDTHeader = {{ sig:{:?},length='{}',rev={},checksum={},  oemid={:?},oem_table_id={:?},oem_revision={}, creator_id={:#x}, creator_revision={} }}",
			from_utf8(&self.signature), self.length, self.revision, self.checksum,
			from_utf8(&self.oemid), from_utf8(&self.oem_table_id), self.oem_revision,
			self.creator_id, self.creator_revision)
	}
}

impl<T> SDTHandle<T>
{
	/// Map an SDT into memory, given a physical address
	pub fn new(physaddr: u64) -> SDTHandle<T>
	{
		//log_trace!("new(physaddr={:#x})", physaddr);
		let ofs = (physaddr & (::PAGE_SIZE - 1) as u64) as usize;
		
		// Obtain length (and validate)
		// TODO: Support the SDT header spanning acrosss two pages
		assert!(::PAGE_SIZE - ofs >= ::core::mem::size_of::<SDTHeader>());
		// Map the header into memory temporarily (maybe)
		let mut handle = match ::memory::virt::map_hw_ro(physaddr - ofs as u64, 1, "ACPI") {
			Ok(v) => v,
			Err(_) => panic!("Oops, temp mapping SDT failed"),
			};
		let (length,) = {
			let hdr = handle.as_ref::<SDTHeader>(ofs);
			
			// Get the length
			(hdr.length as usize,)
			};
		
		// Map the resultant memory
		let npages = (ofs + length + ::PAGE_SIZE - 1) / ::PAGE_SIZE;
		log_trace!("npages = {}, ofs = {}, length = {}", npages, ofs, length);
		if npages != 1
		{
			handle = match ::memory::virt::map_hw_ro(physaddr - ofs as u64, npages, "ACPI") {
				Ok(x) => x,
				Err(_) => panic!("Map fail")
				};
		}
		SDTHandle {
			maphandle: handle,
			ofs: ofs,
			_type: ::core::marker::PhantomData,
			}
	}
	
	pub fn make_static(self) -> &'static SDT<T>
	{
		self.maphandle.make_static::<SDT<T>>(self.ofs)
	}
}

impl<T> ::core::ops::Deref for SDTHandle<T>
{
	type Target = SDT<T>;
	fn deref<'s>(&'s self) -> &'s SDT<T> {
		self.maphandle.as_ref(self.ofs)
	}
}

impl<T> SDT<T>
{
	fn validate(&self) -> bool
	{
		if ::core::mem::size_of::<Self>() != self.header.length as usize {
			log_notice!("SDT size mismatch {} != sizeof({}) {}",
				self.header.length, type_name!(SDT<T>), ::core::mem::size_of::<Self>());
		}
		unsafe {
			let bytes = ::core::slice::from_raw_parts(self as *const _ as *const u8, self.header.length as usize);
			bytes.iter().fold(0, |a,&b| a+b) == 0
		}
	}
	//fn signature<'s>(&'s self) -> &'s str
	//{
	//	from_utf8(self.header.signature).unwrap()
	//}
	fn raw_signature(&self) -> [u8; 4]
	{
		CHECKMARK!();
		self.header.signature
	}
	pub fn data_len(&self) -> usize
	{
		self.header.length as usize - ::core::mem::size_of::<SDTHeader>()
	}
	pub fn data<'s>(&'s self) -> &'s T
	{
		&self.data
	}
	
	pub unsafe fn data_byte_slice(&self) -> &[u8] {
		::core::slice::from_raw_parts(&self.data as *const _ as *const u8, self.data_len())
	}
}

// vim: ft=rust