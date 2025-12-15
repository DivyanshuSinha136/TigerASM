# String Length (ARM64)
from tigerasm import TigerASM

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
