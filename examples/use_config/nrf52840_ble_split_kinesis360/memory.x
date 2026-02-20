MEMORY
{
  /* NOTE 1 K = 1 KiB = 1024 bytes */
  /* Kinesis Adv360 Pro with Adafruit nRF52 bootloader */
  /* App starts at 0x1000, bootloader occupies 0xF4000-0xFFFFF */
  FLASH : ORIGIN = 0x00001000, LENGTH = 1020K
  RAM : ORIGIN = 0x20000008, LENGTH = 255K
}
