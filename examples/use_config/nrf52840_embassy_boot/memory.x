/*
Flash layout matching bootymcbootface nRF layout:
  bootloader: 24K at 0x0, state: 4K at 0x6000
  ACTIVE (this firmware): 432K at 0x7000
  DFU: 436K at 0x73000
  storage: 128K at 0xE0000
*/
MEMORY {
	FLASH : ORIGIN = 0x00007000, LENGTH = 432K
	RAM   : ORIGIN = 0x20000000, LENGTH = 256K
}
