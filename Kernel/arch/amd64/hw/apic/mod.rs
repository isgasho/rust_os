// "Tifflin" Kernel
// - By John Hodge (thePowersGang)
//
// arch/amd64/hw/apic/mod.rs
// - x86 APIC (Advanced Programmable Interrupt Controller) driver
//
// mod.rs -- Core API / init
use _common::*;

module_define!(APIC, [ACPI], init)

mod raw;
mod init;

#[deriving(Default)]
pub struct IRQHandle
{
	num: uint,
	isr_handle: ::arch::interrupts::ISRHandle,
}


#[link_section="processor_local"]
static mut s_lapic_lock: ::sync::Mutex<()> = mutex_init!( () );
static mut s_lapic: *const raw::LAPIC = 0 as *const _;
static mut s_ioapics: *mut Vec<raw::IOAPIC> = 0 as *mut _;

fn init()
{
	let handles = ::arch::acpi::find::<init::ACPI_MADT>("APIC");
	if handles.len() == 0 {
		log_warning!("No MADT ('APIC') table in ACPI");
		return ;
	}
	if handles.len() > 1 {
		log_notice!("Multiple MADTs ({})", handles.len());
	}
	
	let madt = &handles[0];
	madt.data().dump(madt.data_len());
	
	// Handle legacy (8259) PIC
	if (madt.data().flags & 1) != 0 {
		log_notice!("Legacy PIC present, disabling");
		// Disable legacy PIC by masking all interrupts off
		unsafe {
			::arch::x86_io::outb(0xA1, 0xFF);	// Disable slave
			::arch::x86_io::outb(0x21, 0xFF);	// Disable master
		}
	}
	
	// Find the LAPIC address
	let mut lapic_addr = madt.data().local_controller_addr as u64;
	for ent in madt.data().records(madt.data_len()).filter_map(
		|r| match r {
			init::DevLAPICAddr(x) => Some(x.address),
			_ => None
			}
		)
	{
		lapic_addr = ent;
	}
	
	// Create instances of the IOAPIC "driver" for all present controllers
	let ioapics: Vec<_> = madt.data().records(madt.data_len()).filter_map(
			|r|match r {
				init::DevIOAPIC(a) => Some(raw::IOAPIC::new(a.address as u64, a.interrupt_base as uint)),
				_ => None
				}
			).collect();
	
	// Create APIC and IOAPIC instances
	unsafe {
		s_lapic = ::memory::heap::alloc( raw::LAPIC::new(lapic_addr) ) as *const _;
		s_ioapics = ::memory::heap::alloc( ioapics ) as *mut _;
		asm!("sti");
		};
	
}

fn get_ioapic(interrupt: uint) -> Option<&'static mut raw::IOAPIC>
{
	unsafe {
		(*s_ioapics).iter_mut().find( |a| (*a).contains(interrupt) )
	}
}

//
pub fn register_msi(callback: fn (*const()), info: *const ()) -> Result<(uint,::arch::interrupts::ISRHandle),()>
{
	// 1. Find a spare ISR slot on a processor
	let lapic_id = 0;
	let isrnum = 33u;
	// 2. Bind
	let h = try!(::arch::interrupts::bind_isr(lapic_id, isrnum as u8, callback, info));
	Ok( (isrnum, h) )
}

/// Registers an interrupt
pub fn register_irq(global_num: uint, callback: fn (*const()), info: *const() ) -> Result<IRQHandle,()>
{
	// Locate the relevant apic
	let ioapic = match get_ioapic(global_num) {
		Some(x) => x,
		None => return Err( () ),
		};
	
	// 1. Pick a low-loaded processor? 
	// Bind ISR
	let isrnum = 32u;
	let lapic_id = 0;
	let isr_handle = try!( ::arch::interrupts::bind_isr(lapic_id, isrnum as u8, callback, info) );

	// Enable the relevant IRQ on the LAPIC and IOAPIC
	let ofs = global_num - ioapic.first();
	ioapic.set_irq(ofs, isrnum as u8, lapic_id as uint, raw::TriggerEdgeHi);
	
	Ok( IRQHandle {
		num: global_num,
		isr_handle: isr_handle,
		} )
}

impl ::core::fmt::Show for IRQHandle
{
	fn fmt(&self, f: &mut ::core::fmt::Formatter) -> Result<(),::core::fmt::FormatError>
	{
		let ioapic = get_ioapic(self.num).unwrap();
		let ofs = self.num - ioapic.first();
		write!(f, "IRQHandle{{#{}, LAPIC={}, Reg={:#x}}}",
			self.num,
			unsafe { (*s_lapic).get_vec_status(self.isr_handle.idx()) },
			ioapic.get_irq_reg(ofs))
	}
}

impl ::core::ops::Drop for IRQHandle
{
	fn drop(&mut self)
	{
		let ioapic = get_ioapic(self.num).unwrap();
		let ofs = self.num - ioapic.first();
		ioapic.disable_irq(ofs);
	}
}

// vim: ft=rust

