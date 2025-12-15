# Memory Information
from tigerasm import TigerASM

asm = TigerASM()

# Get total memory size
size = asm.memory_size()
print(f"Total memory: {size} bytes")

# Get raw memory pointer (advanced)
ptr = asm.get_memory_ptr()
print(f"Memory base address: {ptr:#x}")
