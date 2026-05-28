#![no_std]
#![no_main]

use core::ptr::addr_of_mut;
use core::sync::atomic::{AtomicU32, Ordering};
use defmt_rtt as _; // Streams logs over the RTT channel to your debug host
use panic_probe as _; // Catches Rust panics and logs the stack trace over RTT

pub mod buffer;
pub mod ingestion;
pub mod pulsar;

// Stack size constant to avoid referencing mutable static structures for size calculations
const CORE1_STACK_SIZE: usize = 4096;

// Shared synchronization barrier between Core 0 and Core 1.
pub static BLOCK_READY: AtomicU32 = AtomicU32::new(0);

// Statically allocated stack for Core 1.
#[link_section = ".uninit"]
static mut CORE1_STACK: [u8; CORE1_STACK_SIZE] = [0; CORE1_STACK_SIZE];

#[cortex_m_rt::entry]
fn main() -> ! {
    defmt::info!(
        "PulsarSync-Core v{} — Core 0 alive",
        env!("CARGO_PKG_VERSION")
    );
    defmt::info!("Target pulsar: PSR B0833-45 (Vela), period=89.328ms, DM=67.97");

    // Launch Core 1 using raw pointers and constant stack sizes
    unsafe {
        let stack_ptr = addr_of_mut!(CORE1_STACK) as *mut u8;
        launch_core1(core1_entry, stack_ptr, CORE1_STACK_SIZE);
    }

    defmt::info!("Core 1 launched successfully — Entering main loop");

    loop {
        cortex_m::asm::wfe();
    }
}

/// Core 1 entry point: Runs the folding loop
#[no_mangle]
pub extern "C" fn core1_entry() -> ! {
    defmt::info!("Core 1 alive — Phase folding engine ready");

    loop {
        while BLOCK_READY.load(Ordering::Acquire) == 0 {
            cortex_m::asm::nop();
        }

        defmt::trace!("Core 1: Block ready signal received, processing...");

        BLOCK_READY.store(0, Ordering::Release);
    }
}

/// Boots Core 1 via the RP2040 SIO FIFO protocol (§2.8.2 datasheet)
unsafe fn launch_core1(entry: extern "C" fn() -> !, stack_ptr: *mut u8, stack_len: usize) {
    let sio_fifo_wr = 0xD000_0050 as *mut u32;
    let sio_fifo_st = 0xD000_0058 as *const u32;

    // Stacks grow downwards, so stack pointer starts at the top (end) of the stack memory block
    let stack_top = stack_ptr.add(stack_len) as u32;

    // Cast function pointer to usize first to satisfy pointer size checks, then to u32
    let entry_ptr = entry as usize as u32;

    let boot_sequence = [0, 0, 1, stack_top, entry_ptr];

    for &val in boot_sequence.iter() {
        // Read the SIO FIFO status register using volatile read to prevent compiler cache optimizations
        while (core::ptr::read_volatile(sio_fifo_st) & 0b10) == 0 {
            cortex_m::asm::nop();
        }
        sio_fifo_wr.write_volatile(val);
        cortex_m::asm::sev();
    }

    defmt::debug!(
        "Core 1 FIFO sequence written, stack_top=0x{:08X}",
        stack_top
    );
}
