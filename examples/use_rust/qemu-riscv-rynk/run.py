import socket, struct, subprocess, sys, time

PORT, ELF = 9000, "target/riscv32imac-unknown-none-elf/debug/rmk-qemu-riscv"
H = struct.Struct("<H B H")

def recv_exact(s, n):
    buf = b""
    while len(buf) < n:
        c = s.recv(n - len(buf))
        if not c: raise ConnectionError
        buf += c
    return buf

subprocess.run(["cargo", "build"], check=True)

q = subprocess.Popen(
    ["qemu-system-riscv32", "-M", "virt", "-cpu", "rv32",
     "-semihosting", "-nographic", "-bios", "none",
     "-kernel", ELF, "-serial", f"tcp::{PORT},server,nowait"],
    stdin=subprocess.DEVNULL, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
print(f'Qemu started, serial on tcp::{PORT}')

s = socket.create_connection(("127.0.0.1", PORT), timeout=5)
s.sendall(H.pack(0x0001, 1, 0))
_, _, n = H.unpack(recv_exact(s, 5))
v = recv_exact(s, n)[1:3]
print(f"Rynk v{v[0]}.{v[1]} OK", flush=True)
s.close()

print("Ctrl+C to stop.", flush=True)
try:
    q.wait()
except KeyboardInterrupt:
    q.terminate()
    q.wait()
