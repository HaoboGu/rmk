/*
This memory.x is for the RP2040 with 2MB flash.
For different flash sizes uncomment below. 
Be aware you need to change FLASH_SIZE in src/central.rs and src/peripheral.rs accordingly
and flash the matching bootloader.
*/
MEMORY {
FLASH : ORIGIN = 0x10007000, LENGTH = 944K
/* 4MB: FLASH : ORIGIN = 0x10007000, LENGTH = 1968K */
/* 8MB: FLASH : ORIGIN = 0x10007000, LENGTH = 4016K */
/* 16MB: FLASH : ORIGIN = 0x10007000, LENGTH = 8112K */
RAM   : ORIGIN = 0x20000000, LENGTH = 256K
}
