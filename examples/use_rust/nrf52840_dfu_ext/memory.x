/* rmk-memory.x for nRF52840 with external-flash DFU (dfu_ext)
 *
 * Provides both the MEMORY layout (absolute flash addresses) and
 * flash-relative DFU symbols consumed by init_flash_from_linkerscript().
 *
 * With dfu_ext the DFU download slot lives on the external SPI flash, so no
 * DFU partition is carved out of the internal flash: the ACTIVE region
 * expands to fill the freed space (0x7F000..0xF8000 on top of the default
 * layout). The __rmk_boot_dfu_* symbols are still read by RMK but unused.
 *
 * If your board has a different flash size, replace this file with the
 * matching file from the rmk-boot dfu_ext releases:
 *   https://github.com/rmk-rs/rmk-boot/releases
 */

MEMORY {
  FLASH : ORIGIN = 0x00007000, LENGTH = 987136   /* ACTIVE region, no DFU */
  RAM   : ORIGIN = 0x20000000, LENGTH = 256K
}

/* DFU partition symbols — offsets relative to flash start */
__rmk_boot_state_offset   = 0x6000;
__rmk_boot_state_size     = 0x1000;
__rmk_boot_dfu_offset     = 0;      /* unused with dfu_ext */
__rmk_boot_dfu_size       = 0;      /* unused with dfu_ext */
__rmk_boot_storage_offset = 0xF8000;
__rmk_boot_storage_size   = 0x8000;