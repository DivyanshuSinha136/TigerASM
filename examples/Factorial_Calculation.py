# Factorial Calculation
from tigerasm import TigerASM

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
