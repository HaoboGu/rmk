MEMORY
{
  /* FLASH and RAM are mandatory memory regions */

  /* STM32H742xI/743xI/753xI       */
  /* STM32H745xI/747xI/755xI/757xI */
  /* STM32H7A3xI/7B3xI             */
  /* FLASH  : ORIGIN = 0x08000000, LENGTH = 2M */

  /* STM32H742xG/743xG       */
  /* STM32H745xG/STM32H747xG */
  /* STM32H7A3xG             */
  /* FLASH  : ORIGIN = 0x08000000, LENGTH = 512K */
  /* FLASH1 : ORIGIN = 0x08100000, LENGTH = 512K */

  /* STM32H750xB   */
  /* STM32H7B0     */
  FLASH  : ORIGIN = 0x08000000, LENGTH = 128K

  /* DTCM  */
  RAM    : ORIGIN = 0x20000000, LENGTH = 128K
  

  /* AXISRAM */
  AXISRAM1 : ORIGIN = 0x24000000, LENGTH = 256K
  AXISRAM2 : ORIGIN = 0x24040000, LENGTH = 384K
  AXISRAM3 : ORIGIN = 0x240A0000, LENGTH = 384K

  /* SRAM */
  AHBSRAM : ORIGIN = 0x30000000, LENGTH = 128K

  /* Backup SRAM */
  /* BSRAM : ORIGIN = 0x38800000, LENGTH = 4K */

  /* Instruction TCM */
  ITCMRAM  : ORIGIN = 0x00000000, LENGTH = 64K

  /* Flash */
  FLASH1 : ORIGIN = 0x90000000, LENGTH = 4096K
}

/* The location of the stack can be overridden using the
   `_stack_start` symbol.  Place the stack at the end of RAM */
_stack_start = ORIGIN(RAM) + LENGTH(RAM);

/* The location of the .text section can be overridden using the
   `_stext` symbol.  By default it will place after .vector_table */
/* _stext = ORIGIN(FLASH) + 0x40c; */

/* These sections are used for some of the examples */
SECTIONS {
  .axisram1 (NOLOAD) : ALIGN(8) {
    *(.axisram1 .axisram1.*);
    . = ALIGN(8);
    } > AXISRAM1
  .axisram2 (NOLOAD) : ALIGN(8) {
    *(.axisram2 .axisram2.*);
    . = ALIGN(8);
    } > AXISRAM2
  .axisram3 (NOLOAD) : ALIGN(8) {
    *(.axisram3 .axisram3.*);
    . = ALIGN(8);
    } > AXISRAM3

  .ahbsram (NOLOAD) : ALIGN(4) {
    *(.ahbsram .ahbsram.*);
    . = ALIGN(4);
    } > AHBSRAM
};