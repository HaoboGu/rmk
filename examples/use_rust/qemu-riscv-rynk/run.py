from pathlib import Path
import subprocess

PORT = 9000
ROOT = Path(__file__).resolve().parents[3]
HERE = Path(__file__).resolve().parent
ELF = HERE / "target/riscv32imac-unknown-none-elf/debug/rmk-qemu-riscv"

subprocess.run(["cargo", "build"], cwd=HERE, check=True)

q = subprocess.Popen(
    ["qemu-system-riscv32", "-M", "virt", "-cpu", "rv32",
     "-semihosting", "-nographic", "-bios", "none",
     "-kernel", str(ELF), "-serial", f"tcp::{PORT},server,nowait"],
    stdin=subprocess.DEVNULL, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
print(f"QEMU started, serial on tcp::{PORT}", flush=True)

try:
    subprocess.run(
        [
            "cargo",
            "run",
            "--manifest-path",
            str(ROOT / "rynk/Cargo.toml"),
            "--example",
            "qemu_behavior",
            "--",
            f"127.0.0.1:{PORT}",
        ],
        cwd=ROOT,
        check=True,
    )
finally:
    q.terminate()
    try:
        q.wait(timeout=2)
    except subprocess.TimeoutExpired:
        q.kill()
        q.wait()
