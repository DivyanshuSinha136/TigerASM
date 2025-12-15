# Simple Loop

from tigerasm import TigerASM
from time import time as t


# -------- Python Inline Assembly ----------

asm = TigerASM()

code = """
    mov rax, 0
    mov rbx, 10

loop:
    add rax, rbx
    dec rbx
    jnz loop
    ret
"""

asm.asm(code)
start = t()
asm.execute()
stop = t() - start
print(f"RAX = {asm.get('rax')}, Take = {stop}s to run")
asm.clear()
