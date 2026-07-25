# RMK QEMU (RISC-V + Rynk)

```sh
python run.py
```

RMK firmware on QEMU RISC-V `virt`, UART -> TCP :9000.
The runner builds the firmware, starts QEMU, then runs the strict Rynk behavior
verifier in `rynk/examples/qemu_behavior.rs`.

To run the focused concurrent-request regression probe over the same QEMU UART:

```sh
python run.py --concurrent-repro
```

The probe first verifies the two commands sequentially, then issues them
concurrently through the public host API. It exits non-zero if either request
does not complete within one second.
