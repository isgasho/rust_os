//
//
//

pub mod addresses
{
	pub static heap_start:     uint = 0xFFFF8000_00000000;
	pub static heap_end:       uint = 0xFFFF9000_00000000;
	pub static modules_start:  uint = 0xFFFF9000_00000000;
	pub static modules_end:    uint = 0xFFFFA000_00000000;
	pub static physinfo_start: uint = 0xFFFFA000_00000000;
	pub static physinfo_end:   uint = 0xFFFFB000_00000000;	// TODO: Needed?
}

// vim: ft=rust
