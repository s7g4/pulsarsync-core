use core::sync::atomic::{AtomicUsize, Ordering};

pub const BLOCK_SIZE: usize = 512; // Bytes per sample block
pub const CAPACITY: usize = 64; // Ring slots — must be a power of 2

// Compile-time assertion to enforce power-of-two constraints
const _: () = assert!(CAPACITY.is_power_of_two(), "CAPACITY must be a power of 2");

/// A single block of raw ADC samples
#[repr(C, align(4))] // Forces 4-byte boundary alignment. This is critical for 32-bit word loads.
pub struct SampleBlock {
    pub samples: [u8; BLOCK_SIZE],
    pub timestamp_ticks: u64, // Hardware timer tick at block start
    pub block_index: u32,     // Monotonic sequence number (detects drops)
}

/// The static block pool — all memory for the pipeline resides here.
/// Since it is static, this memory is guaranteed to be available at boot.
static mut BLOCK_POOL: [SampleBlock; CAPACITY] = {
    const ZERO_BLOCK: SampleBlock = SampleBlock {
        samples: [0u8; BLOCK_SIZE],
        timestamp_ticks: 0,
        block_index: 0,
    };
    [ZERO_BLOCK; CAPACITY]
};

pub struct RingBuffer {
    head: AtomicUsize, // Core 1 (consumer) advances this
    tail: AtomicUsize, // Core 0 (producer) advances this
}

impl Default for RingBuffer {
    fn default() -> Self {
        Self::new()
    }
}

impl RingBuffer {
    pub const fn new() -> Self {
        Self {
            head: AtomicUsize::new(0),
            tail: AtomicUsize::new(0),
        }
    }

    /// Acquire a writable slot (called by Core 0).
    /// Returns a mutable reference directly pointing inside the static pool, or None if full.
    /// Zero-copy: We return a reference, avoiding memcpy.
    pub fn acquire_write_slot(&self) -> Option<&'static mut SampleBlock> {
        let tail = self.tail.load(Ordering::Relaxed);
        let head = self.head.load(Ordering::Acquire); // Synchronize with Core 1's release

        // If tail has wrapped all the way around and caught up with head, the buffer is full
        if tail.wrapping_sub(head) >= CAPACITY {
            return None;
        }

        // Fast bitwise AND replaces division modulo operations on Cortex-M0+
        let slot = tail & (CAPACITY - 1);
        Some(unsafe { &mut BLOCK_POOL[slot] })
    }

    /// Commit a written slot — advances tail so Core 1 can see it.
    pub fn commit_write(&self) {
        let tail = self.tail.load(Ordering::Relaxed);
        // Release: All memory updates to the slot are visible before tail is incremented
        self.tail.store(tail.wrapping_add(1), Ordering::Release);
    }

    /// Acquire a readable slot (called by Core 1).
    /// Returns a read-only reference directly pointing inside the pool, or None if empty.
    pub fn acquire_read_slot(&self) -> Option<&'static SampleBlock> {
        let head = self.head.load(Ordering::Relaxed);
        let tail = self.tail.load(Ordering::Acquire); // Synchronize with Core 0's release

        if head == tail {
            return None; // Buffer is empty
        }

        let slot = head & (CAPACITY - 1);
        Some(unsafe { &BLOCK_POOL[slot] })
    }

    /// Release a read slot — advances head so Core 0 can reuse the slot memory.
    pub fn release_read(&self) {
        let head = self.head.load(Ordering::Relaxed);
        // Release: Core 1 is finished reading the slot before head is incremented
        self.head.store(head.wrapping_add(1), Ordering::Release);
    }

    /// Current occupancy (diagnostic only, not used in performance-critical paths)
    pub fn occupancy(&self) -> usize {
        let tail = self.tail.load(Ordering::Relaxed);
        let head = self.head.load(Ordering::Relaxed);
        tail.wrapping_sub(head)
    }
}

// Global static instance of the Ring Buffer
pub static RING: RingBuffer = RingBuffer::new();
