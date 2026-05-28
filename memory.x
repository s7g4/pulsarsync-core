/* RP2040 Memory Layout */
MEMORY
{
  /* 2MB external Flash mapped via XIP (eXecute In Place) */
  FLASH : ORIGIN = 0x10000000, LENGTH = 2048K
  /* 264KB SRAM divided into banks. Standard layout treats it as one contiguous block */
  RAM   : ORIGIN = 0x20000000, LENGTH = 264K
}
