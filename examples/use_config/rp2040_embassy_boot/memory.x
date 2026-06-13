/*
This memory.x is for the RP2040 with 2MB flash.
For different flash sizes see docs. 
*/
MEMORY {
	FLASH : ORIGIN = 0x10007000, LENGTH = 944K
	RAM   : ORIGIN = 0x20000000, LENGTH = 256K
}
