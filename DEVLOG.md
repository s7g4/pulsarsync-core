# Developer Log (DEVLOG)
## Phase 0: Environment & Toolchain Bootstrap
### Goal
Establish a deterministic cross-compilation environment targeting the bare-metal dual-core Cortex-M0+ architecture.
### What Broke & The Fight
* **Linker Error (Section `.vector_table` not found)**: On the first compilation run, the build failed because the default linker script didn't know the flash origin.
  * *Fix*: Implemented `memory.x` defining flash and RAM origins exactly at `0x10000000` and `0x20000000` and configured `rustflags` in `.cargo/config.toml` to pass `-Tlink.x` to the compiler.
* **Stack Overflow Risks**: Dual-core bare-metal setups can silently crash if a stack overflow corrupts the BSS segment.
  * *Fix*: Pinned `flip-link` in cargo runner flags to reorder variables, placing the stack pointer at the bottom of the RAM boundary so any stack overflow triggers a hardware boundary fault rather than silent data corruption.
### Status
Environment bootstrapped, build configs validated, documentation registry initialized.
