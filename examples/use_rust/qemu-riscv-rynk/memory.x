/* QEMU `virt` machine: 16 MB RAM at 0x80000000.
 * With `-bios none`, the kernel ELF is loaded directly at RAM start.
 * Use only 16 MB — QEMU places the device tree near the top of RAM.
 *
 * riscv-rt's link.x provides defaults: _max_hart_id=0, _hart_stack_size=2K,
 * _stack_start = ORIGIN(REGION_STACK) + LENGTH(REGION_STACK).
 */
MEMORY
{
  RAM : ORIGIN = 0x80000000, LENGTH = 16M
}

REGION_ALIAS("REGION_TEXT", RAM);
REGION_ALIAS("REGION_RODATA", RAM);
REGION_ALIAS("REGION_DATA", RAM);
REGION_ALIAS("REGION_BSS", RAM);
REGION_ALIAS("REGION_HEAP", RAM);
REGION_ALIAS("REGION_STACK", RAM);
