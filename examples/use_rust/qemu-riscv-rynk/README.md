# RMK QEMU (RISC-V + Rynk)

```sh
python run.py
```

RMK firmware on QEMU RISC-V `virt`, UART -> TCP :9000.
The runner builds the firmware, starts QEMU, then runs the strict Rynk behavior
verifier in `rynk/examples/qemu_behavior.rs`.
