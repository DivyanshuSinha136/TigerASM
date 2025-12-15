# Bulk Memory Operations
from tigerasm import TigerASM

asm = TigerASM()

# Load binary file to memory
bytes_read = asm.load_binary_file("data.bin", 0x10000)
print(f"Loaded {bytes_read} bytes")

# Load bytes from Python
data = bytes([0x48, 0x89, 0xC3, 0xC3])  # mov rbx, rax; ret
asm.load_bytes_to_memory(data, 0x20000)

# Write bytes to memory
asm.write_bytes(0x30000, [0x90] * 10)  # 10 NOPs

# Read bytes from memory
data = asm.read_bytes(0x30000, 10)
print(f"Read: {data}")

# Save memory region to file
asm.save_memory_to_file("output.bin", 0x10000, 1024)

# Clear memory (optional range)
asm.clear_memory(0x1000, 512)  # Clear specific region
asm.clear_memory()              # Clear all memory
