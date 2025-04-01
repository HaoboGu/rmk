MEMORY
{
  FLASH : ORIGIN = 0x00000000, LENGTH = 512K
  RAM : ORIGIN = 0x20000000, LENGTH = 64K

  /* These values correspond to the nRF52832 WITH Adafruit nRF52 bootloader */
  /* FLASH : ORIGIN = 0x00001000, LENGTH = 508K */
  /* RAM : ORIGIN = 0x20000008, LENGTH = 63K */
}