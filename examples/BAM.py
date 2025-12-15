# Binary Access Memory

from tigerasm import TigerASM

asm = TigerASM()

# Write to memory
asm.write_mem(0x1000, 0xDEADBEEF, 4)  # address, value, size

# Read from memory
value = asm.read_mem(0x1000, 4)  # address, size
print(f"Value at 0x1000: {value:#x}")

# Supported sizes: 1, 2, 4, 8 bytes
asm.write_mem(0x2000, 0xFF, 1)        # byte
asm.write_mem(0x2001, 0xFFFF, 2)      # word
asm.write_mem(0x2003, 0xFFFFFFFF, 4)  # dword
asm.write_mem(0x2007, 0xFFFFFFFFFFFFFFFF, 8)  # qword
