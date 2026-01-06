# TigerASM 🐯
[![CodeQL Advanced](https://github.com/DivyanshuSinha136/TigerASM/actions/workflows/codeql.yml/badge.svg)](https://github.com/DivyanshuSinha136/TigerASM/actions/workflows/codeql.yml)

**Ultimate Complete Runtime Assembler for Python**

TigerASM is a high-performance runtime assembler library for Rust with Python bindings.  
It supports x86-64 and ARM64 architectures, allowing you to execute dynamic assembly instructions from Python.

## ✨ Features

- **349 Total Registers Support**
  - 122 x86-64 registers (GPR, XMM, YMM, ZMM, FPU, segment, control, debug)
  - 227 ARM64 registers (general purpose, SIMD, system)
- **Complete Memory Addressing**
  - `[reg]`, `[reg+offset]`, `[reg+reg*scale+disp]`
  - Intel and AT&T syntax support
- **Multiple Number Formats**
  - Decimal: `42`
  - Hexadecimal: `0x2A`
  - Binary: `0b101010`
- **Advanced Memory Management**
  - Built-in allocator with coalescing
  - 10MB default memory pool
  - Read/write operations with multiple sizes
- **Both Architectures Supported**
  - x86-64 (Intel/AMD)
  - AArch64 (ARM64)

## 📦 Installation

```bash
pip install tigerasm
```

## 🏗️ Architecture Support

### x86-64 Architecture

Supported on Intel and AMD processors with full instruction set support.

**Available Registers:**
- **General Purpose:** `rax`, `rbx`, `rcx`, `rdx`, `rsi`, `rdi`, `rsp`, `rbp`, `r8`-`r15`
- **32-bit:** `eax`, `ebx`, `ecx`, `edx`, `esi`, `edi`, `esp`, `ebp`, `r8d`-`r15d`
- **16-bit:** `ax`, `bx`, `cx`, `dx`, `si`, `di`, `sp`, `bp`, `r8w`-`r15w`
- **8-bit:** `al`, `ah`, `bl`, `bh`, `cl`, `ch`, `dl`, `dh`, `sil`, `dil`, `bpl`, `spl`, `r8b`-`r15b`
- **SIMD:** `xmm0`-`xmm15`, `ymm0`-`ymm15`, `zmm0`-`zmm15`
- **FPU:** `st0`-`st7`, `mm0`-`mm7`
- **Segment:** `cs`, `ds`, `es`, `fs`, `gs`, `ss`
- **Control:** `cr0`-`cr4`
- **Debug:** `dr0`-`dr7`
- **Special:** `rip`, `rflags`, `gdtr`, `idtr`, `ldtr`, `tr`

### ARM64 (AArch64) Architecture

Supported on ARM processors including Apple Silicon, AWS Graviton, and more.

**Available Registers:**
- **General Purpose:** `x0`-`x30` (64-bit), `w0`-`w30` (32-bit)
- **Special:** `sp`, `pc`, `lr` (x30), `fp` (x29), `xzr`/`wzr` (zero register)
- **SIMD/FP:** `v0`-`v31`, `q0`-`q31`, `d0`-`d31`, `s0`-`s31`, `h0`-`h31`, `b0`-`b31`
- **System:** `pstate`, `cpsr`, `apsr`
- **Exception Levels:** `sp_el0`-`sp_el3`, `elr_el1`-`elr_el3`, `spsr_el1`-`spsr_el3`
- **FP Control:** `fpcr`, `fpsr`

## 🚀 Quick Start

### Basic Usage

```python
from tigerasm import TigerASM

# Create assembler instance
asm = TigerASM()
asm.setup("x86_64")  # or "aarch64" for ARM64

# Write assembly code
asm.asm("""
    mov rax, 42
    add rax, 8
    ret
""")

# Execute and get result
asm.execute()
result = asm.get("rax")  # Returns 50
print(f"Result: {result}")
```

### Working with Different Number Formats

```python
asm = TigerASM()
asm.setup()

# Decimal, hexadecimal, and binary
asm.asm("""
    mov rax, 100        ; Decimal
    mov rbx, 0xFF       ; Hexadecimal
    mov rcx, 0b1010     ; Binary
    add rax, rbx
    add rax, rcx
    ret
""")

asm.execute()
print(f"Sum: {asm.get('rax')}")  # 100 + 255 + 10 = 365
```

## 📝 Instruction Set

### Data Movement

```python
# Intel syntax (default)
mov rax, 100              # Register <- Immediate
mov rbx, rax              # Register <- Register
mov rax, [rbx]            # Register <- Memory
mov [rbx], rax            # Memory <- Register
lea rax, [rbx + rcx*4]    # Load Effective Address

# AT&T syntax (also supported)
movq $100, %rax           # AT&T immediate syntax
movq %rax, %rbx           # AT&T register syntax
movq (%rbx), %rax         # AT&T memory syntax
```

### Arithmetic Operations

```python
add rax, 10               # Addition
sub rax, 5                # Subtraction
mul rbx                   # Unsigned multiplication (rax * rbx)
imul rax, rbx             # Signed multiplication
div rcx                   # Unsigned division
inc rax                   # Increment
dec rax                   # Decrement
neg rax                   # Negate
```

### Logical Operations

```python
and rax, 0xFF             # Bitwise AND
or rax, 0x0F              # Bitwise OR
xor rax, rbx              # Bitwise XOR
not rax                   # Bitwise NOT
test rax, rbx             # Test (AND without storing)
```

### Bit Manipulation

```python
# x86-64
shl rax, 2                # Shift left
shr rax, 1                # Shift right (logical)
sal rax, 3                # Shift arithmetic left
sar rax, 2                # Shift arithmetic right
rol rax, 4                # Rotate left
ror rax, 3                # Rotate right
rcl rax, 1                # Rotate through carry left
rcr rax, 2                # Rotate through carry right

# ARM64
lsl x0, x0, #2            # Logical shift left
lsr x0, x0, #1            # Logical shift right
asr x0, x0, #3            # Arithmetic shift right
```

### Control Flow

```python
# Labels and jumps
asm.asm("""
loop_start:
    dec rcx
    jnz loop_start        # Jump if not zero
    ret
""")

# Conditional jumps (x86-64)
je label                  # Jump if equal
jne label                 # Jump if not equal
jg label                  # Jump if greater
jge label                 # Jump if greater or equal
jl label                  # Jump if less
jle label                 # Jump if less or equal
ja label                  # Jump if above (unsigned)
jb label                  # Jump if below (unsigned)

# ARM64 branches
b label                   # Unconditional branch
beq label                 # Branch if equal
bne label                 # Branch if not equal
bgt label                 # Branch if greater than
blt label                 # Branch if less than
bl function               # Branch with link (call)
```

## 🧠 Register Management

### Setting Register Values

```python
asm = TigerASM()

# x86-64 registers
asm.mov("rax", 0x1234567890ABCDEF)
asm.mov("rbx", 42)
asm.mov("rcx", 0b11111111)

# ARM64 registers
asm.mov("x0", 100)
asm.mov("x1", 200)
asm.mov("sp", 0x7fff0000)
```

### Reading Register Values

```python
# Get specific register
value = asm.get("rax")
print(f"RAX = {value:#x}")

# Get ARM register
value = asm.get("x0")
print(f"X0 = {value}")

# Default to RAX if no register specified
default_val = asm.get()  # Returns RAX on x86-64, X0 on ARM64
```

### Dumping All Registers

```python
# Display all registers for current architecture
print(asm.dump_regs())

# Force specific architecture dump
print(asm.dump_regs_x86())
print(asm.dump_regs_arm())
```

Output example (x86-64):
```
=== x86-64 Registers ===
RAX=0000000000000064 RBX=00000000000000c8 RCX=0000000000000000 RDX=0000000000000000
RSI=0000000000000000 RDI=0000000000000000 RBP=0000000000000000 RSP=0000000000000000
R8 =0000000000000000 R9 =0000000000000000 R10=0000000000000000 R11=0000000000000000
R12=0000000000000000 R13=0000000000000000 R14=0000000000000000 R15=0000000000000000
RIP=0000000000000000 RFLAGS=0000000000000000
Segment: CS=0000 DS=0000 ES=0000 FS=0000 GS=0000 SS=0000
```

## 💾 Memory Operations

### Basic Memory Access

```python
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
```

### Memory Addressing Modes

```python
# Intel syntax
mov rax, [rbx]                    # [base]
mov rax, [rbx + 8]                # [base + offset]
mov rax, [rbx + rcx]              # [base + index]
mov rax, [rbx + rcx*4]            # [base + index*scale]
mov rax, [rbx + rcx*8 + 16]       # [base + index*scale + disp]
mov rax, [rip + 0x1000]           # RIP-relative

# AT&T syntax
movq (%rbx), %rax                 # (base)
movq 8(%rbx), %rax                # offset(base)
movq (%rbx,%rcx), %rax            # (base,index)
movq (%rbx,%rcx,4), %rax          # (base,index,scale)
movq 16(%rbx,%rcx,8), %rax        # offset(base,index,scale)
```

### Memory Allocation

```python
# Allocate memory block
address = asm.alloc(1024)  # Allocate 1KB
if address == 0:
    print("Allocation failed")
else:
    print(f"Allocated at: {address:#x}")
    
    # Use the memory
    asm.write_mem(address, 0x42, 1)
    
    # Free when done
    asm.free(address)

# Check memory statistics
used, free, blocks = asm.memory_stats()
print(f"Used: {used} bytes, Free: {free} bytes, Blocks: {blocks}")
```

### Bulk Memory Operations

```python
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
```

### Memory Information

```python
# Get total memory size
size = asm.memory_size()
print(f"Total memory: {size} bytes")

# Get raw memory pointer (advanced)
ptr = asm.get_memory_ptr()
print(f"Memory base address: {ptr:#x}")
```

## 📂 Loading Assembly Files

### From String

```python
asm = TigerASM()
code = """
    mov rax, 10
    mov rbx, 20
    add rax, rbx
    ret
"""
asm.asm(code)
asm.execute()
```

### From File

```python
asm = TigerASM()

# Load .asm file
asm.load_file("program.asm")
asm.execute()

# Or use asm() with file handle
with open("program.asm", "rb") as f:
    asm.asm("", file=f)
asm.execute()
```

### Multiple Code Sections

```python
asm = TigerASM()

# Add code incrementally
asm.asm("mov rax, 0")
asm.asm("add rax, 10")
asm.asm("add rax, 20")
asm.asm("ret")

# Execute combined code
asm.execute()
print(asm.get("rax"))  # 30

# Clear and start fresh
asm.clear()
```

## 🔧 Advanced Examples

### Factorial Calculation

```python
asm = TigerASM()
asm.setup("x86_64")

# Calculate factorial of 5
asm.asm("""
    mov rax, 5          ; n = 5
    mov rbx, 1          ; result = 1
    
factorial_loop:
    test rax, rax       ; if n == 0
    jz done             ; goto done
    
    imul rbx, rax       ; result *= n
    dec rax             ; n--
    jmp factorial_loop
    
done:
    mov rax, rbx        ; return result
    ret
""")

asm.execute()
result = asm.get("rax")
print(f"5! = {result}")  # 120
```

### String Length (ARM64)

```python
asm = TigerASM()
asm.setup("aarch64")

# Store string in memory
text = b"Hello, World!\x00"
asm.load_bytes_to_memory(list(text), 0x10000)

asm.asm("""
    mov x0, #0x10000    ; String address
    mov x1, #0          ; Counter
    
strlen_loop:
    ldrb w2, [x0, x1]   ; Load byte
    cbz w2, done        ; If zero, done
    add x1, x1, #1      ; counter++
    b strlen_loop
    
done:
    mov x0, x1          ; Return length
    ret
""")

asm.execute()
length = asm.get("x0")
print(f"String length: {length}")  # 13
```

### Memory Copy

```python
asm = TigerASM()
asm.setup()

# Source data
src_data = list(range(0, 100))
asm.load_bytes_to_memory(src_data, 0x1000)

# Copy routine
asm.asm("""
    mov rsi, 0x1000     ; Source
    mov rdi, 0x2000     ; Destination
    mov rcx, 100        ; Count
    
copy_loop:
    test rcx, rcx
    jz done
    
    mov al, [rsi]       ; Load byte
    mov [rdi], al       ; Store byte
    
    inc rsi
    inc rdi
    dec rcx
    jmp copy_loop
    
done:
    ret
""")

asm.execute()

# Verify
dest_data = asm.read_bytes(0x2000, 100)
print(f"Copy successful: {dest_data == bytes(src_data)}")
```

### Working with SIMD (x86-64)

```python
asm = TigerASM()
asm.setup("x86_64")

# Note: SIMD operations require extended instruction support
# This is a conceptual example
asm.asm("""
    ; Load 4 floats into XMM0
    movaps xmm0, [rsi]
    
    ; Multiply by 2.0
    mulps xmm0, xmm1
    
    ; Store result
    movaps [rdi], xmm0
    ret
""")
```

## 🎯 Architecture Detection

```python
asm = TigerASM()

# Auto-detect (default)
asm.setup()
print(f"Detected: {asm.get_arch()}")

# Force specific architecture
asm.setup("x86_64")   # x86-64
asm.setup("aarch64")  # ARM64

# Alternative names
asm.setup("x86")      # → x86-64
asm.setup("amd64")    # → x86-64
asm.setup("arm64")    # → aarch64
asm.setup("arm")      # → aarch64
```

## 🔍 Validation

```python
asm = TigerASM()

# Check if register is valid
if asm.is_valid_register("rax"):
    print("RAX is valid")

if asm.is_valid_register("x0"):
    print("X0 is valid")

# Check architecture
arch = asm.get_arch()
if arch == "x86_64":
    print("Running on x86-64")
elif arch == "aarch64":
    print("Running on ARM64")
```

## 💾 Saving Executable Code

```python
asm = TigerASM()
asm.asm("""
    mov rax, 42
    ret
""")

# Compile without executing
asm.execute()  # This compiles the code

# Save compiled code to file
asm.install("output.bin")
```

## 🐛 Error Handling

```python
from tigerasm import TigerASM

asm = TigerASM()

try:
    # Invalid register
    asm.mov("invalid_reg", 42)
except ValueError as e:
    print(f"Error: {e}")

try:
    # Invalid instruction
    asm.asm("invalid_instruction rax, rbx")
except ValueError as e:
    print(f"Parse error: {e}")

try:
    # Memory out of bounds
    asm.write_mem(0xFFFFFFFFFFFF, 42, 8)
except ValueError as e:
    print(f"Memory error: {e}")
```

## 📊 Performance Tips

1. **Batch Assembly Code**: Use multi-line strings instead of multiple `asm()` calls
2. **Reuse Instances**: Create one `TigerASM` instance and clear it between uses
3. **Memory Management**: Free allocated memory when done to enable coalescing
4. **Register Usage**: Prefer registers over memory for frequently accessed data

```python
# Good: Single batch
asm.asm("""
    mov rax, 1
    mov rbx, 2
    add rax, rbx
""")

# Less efficient: Multiple calls
asm.asm("mov rax, 1")
asm.asm("mov rbx, 2")
asm.asm("add rax, rbx")
```

## 🔒 Safety Notes

- Memory operations are bounds-checked
- Invalid register names raise `ValueError`
- Memory allocation failures return 0
- Always validate addresses before use
- The memory pool is isolated and safe

## 🤝 Contributing

Contributions are welcome! Please ensure:
- Code follows Rust best practices
- New instructions are documented
- Tests are included for new features
- README is updated

## 📄 License
![License](https://img.shields.io/badge/License-LGPL%20v3-blue.svg)

TigerASM is licensed under the **GNU Lesser General Public License v3.0 or later (LGPL-3.0-or-later)**.  
For full details, see the [LICENSE](./LICENSE) file included in this project.

---

## 🙏 Acknowledgments

This project was made possible with:

- [PyO3](https://pyo3.rs/) – Rust bindings for Python
- [dynasm-rs](https://github.com/CensoredUsername/dynasm-rs) – Dynamic assembly runtime

---

**Happy Assembling! 🐯⚡**
