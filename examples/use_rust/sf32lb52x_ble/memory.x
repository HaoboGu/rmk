# SF32LB52-MOD-1-N16R8
MEMORY
{
  FLASH : ORIGIN = 0x12020000, LENGTH = 16M - 128K
  # Note: The last 1KB of HPSYS RAM is reserved by SDK for inter-core mailbox IPC buffer:
  # - 0x2007FC00..0x2007FDFF (CH2, 512B)
  # - 0x2007FE00..0x2007FFFF (CH1, 512B)
  # Without reserving this region in the linker script, stack/globals may overwrite it,
  # causing random IPC ring buffer corruption.
  RAM : ORIGIN = 0x20000000, LENGTH = 512K - 1K
  PSRAM : ORIGIN = 0x60000000, LENGTH = 8M
}
