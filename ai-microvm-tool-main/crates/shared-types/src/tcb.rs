//! Fixed-size Task Control Block. Slot tetap → slab array, O(1) indexing.
//! Byte 0..44 = layout Symbian-style sesuai desain; 44..64 = reserved supaya
//! satu slot = satu cache line (64 byte) di x86/ARM → lebih cache-friendly.

use crate::VmState;

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VmTcb {
    pub magic: u32,          // 0
    pub version: u16,        // 4
    pub state: u8,           // 6  (VmState as u8)
    pub flags: u8,           // 7
    pub vm_id: u64,          // 8
    pub profile_id: u64,     // 16
    pub generation: u32,     // 24  (counter anti-ABA)
    pub mem_mib: u16,        // 28
    pub vcpu: u8,            // 30
    pub reserved: u8,        // 31
    pub caps_hash: u64,      // 32
    pub created_tick: u32,   // 40
    // --- akhir layout 44-byte desain awal ---
    pub _reserved: [u8; 20], // 44..64  (round ke cache line)
}

pub const VM_TCB_MAGIC: u32 = 0x4149_4D54; // 'AIMT'
pub const VM_TCB_VERSION: u16 = 1;

// Jaminan compile-time: ukuran slot tidak boleh berubah tanpa disadari.
const _: () = assert!(std::mem::size_of::<VmTcb>() == 64);

impl VmTcb {
    pub fn new(vm_id: u64, profile_id: u64, state: VmState) -> Self {
        Self {
            magic: VM_TCB_MAGIC, version: VM_TCB_VERSION, state: state.to_u8(), flags: 0,
            vm_id, profile_id, generation: 1, mem_mib: 0, vcpu: 0, reserved: 0,
            caps_hash: 0, created_tick: 0, _reserved: [0; 20],
        }
    }
    pub fn state(&self) -> Option<VmState> { VmState::from_u8(self.state) }
    pub fn bump_generation(&mut self) { self.generation = self.generation.wrapping_add(1); }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::mem::{align_of, offset_of, size_of};

    #[test]
    fn slot_is_one_cache_line() {
        assert_eq!(size_of::<VmTcb>(), 64);
        assert_eq!(align_of::<VmTcb>(), 8); // array TCB rapat, tanpa padding antar slot
    }

    #[test]
    fn blueprint_prefix_is_44() {
        assert_eq!(offset_of!(VmTcb, created_tick), 40);
        assert_eq!(offset_of!(VmTcb, _reserved), 44); // prefix desain awal utuh
    }

    #[test]
    fn new_sets_magic_and_state() {
        let t = VmTcb::new(7, 99, VmState::Ready);
        assert_eq!(t.magic, VM_TCB_MAGIC);
        assert_eq!(t.vm_id, 7);
        assert_eq!(t.profile_id, 99);
        assert_eq!(t.state(), Some(VmState::Ready));
        assert_eq!(t.generation, 1);
    }

    #[test]
    fn generation_wraps() {
        let mut t = VmTcb::new(1, 1, VmState::Ready);
        t.generation = u32::MAX;
        t.bump_generation();
        assert_eq!(t.generation, 0);
    }
}
