# Add Numbers in Loop

from tigerasm import TigerASM
from time import time as t

asm = TigerASM()

asm.asm("""
    mov rax, 0
    mov rbx, 1000000
loop:
    dec rbx
    jnz loop
    ret
""")

start = t()
asm.execute()
stop = t() - start
print(f"Time to run: {stop}s")
