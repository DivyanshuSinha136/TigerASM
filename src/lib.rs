// TigerASM - Runtime assembler for Python
// Copyright (C) 2025 Divyanshu Sinha
//
// This library is free software; you can redistribute it and/or modify
// it under the terms of the GNU Lesser General Public License as published
// by the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// This library is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.
// See the GNU Lesser General Public License for more details.
//
// You should have received a copy of the GNU Lesser General Public License
// along with this library. If not, see <https://www.gnu.org/licenses/>.

use pyo3::prelude::*;
use pyo3::exceptions::PyValueError;
use dynasmrt::{dynasm, DynasmApi, DynasmLabelApi, ExecutableBuffer, AssemblyOffset};
#[cfg(target_arch = "x86_64")]
use dynasmrt::x64::Assembler as AssemblerX64;
#[cfg(target_arch = "aarch64")]
use dynasmrt::aarch64::Assembler as AssemblerAArch64;
use std::collections::HashMap;
use std::fs::File;
use std::io::{Read, Write};

// ═══════════════════════════════════════════════════════════════════════════
// COMPLETE Register Structures (from lib_ultimate.rs)
// ═══════════════════════════════════════════════════════════════════════════

#[derive(Default, Debug, Clone)]
pub struct RegistersX86Complete {
    pub rax: u64, pub rbx: u64, pub rcx: u64, pub rdx: u64,
    pub rsi: u64, pub rdi: u64, pub rsp: u64, pub rbp: u64,
    pub r8: u64, pub r9: u64, pub r10: u64, pub r11: u64,
    pub r12: u64, pub r13: u64, pub r14: u64, pub r15: u64,
    pub rip: u64, pub rflags: u64,
    pub cs: u16, pub ds: u16, pub es: u16, pub fs: u16, pub gs: u16, pub ss: u16,
    pub cr0: u64, pub cr1: u64, pub cr2: u64, pub cr3: u64, pub cr4: u64,
    pub dr0: u64, pub dr1: u64, pub dr2: u64, pub dr3: u64,
    pub dr4: u64, pub dr5: u64, pub dr6: u64, pub dr7: u64,
    pub gdtr: u64, pub idtr: u64, pub ldtr: u16, pub tr: u16,
    pub xmm: [[u64; 2]; 16],
    pub ymm_high: [[u64; 2]; 16],
    pub zmm_high: [[u64; 4]; 16],
    pub st: [(u64, u16); 8],
    pub mm: [u64; 8],
    pub fpr: [u64; 8],
    pub fpu_control: u16, pub fpu_status: u16, pub fpu_tag: u16,
}

#[derive(Default, Debug, Clone)]
pub struct RegistersARM64Complete {
    pub x: [u64; 31],
    pub sp: u64, pub pc: u64, pub lr: u64, pub fp: u64, pub xzr: u64,
    pub v: [[u64; 2]; 32],
    pub pstate: u64, pub cpsr: u32, pub apsr: u32,
    pub sp_el0: u64, pub sp_el1: u64, pub sp_el2: u64, pub sp_el3: u64,
    pub elr_el1: u64, pub elr_el2: u64, pub elr_el3: u64,
    pub spsr_el1: u32, pub spsr_el2: u32, pub spsr_el3: u32,
    pub fpcr: u32, pub fpsr: u32,
}

// ═══════════════════════════════════════════════════════════════════════════
// Memory Allocator (same as before)
// ═══════════════════════════════════════════════════════════════════════════

#[derive(Debug, Clone)]
struct MemoryBlock {
    start: usize,
    size: usize,
    in_use: bool,
}

struct MemoryAllocator {
    blocks: Vec<MemoryBlock>,
    memory_size: usize,
}

impl MemoryAllocator {
    fn new(size: usize) -> Self {
        Self {
            blocks: vec![MemoryBlock { start: 0, size, in_use: false }],
            memory_size: size,
        }
    }

    fn allocate(&mut self, size: usize) -> Option<usize> {
        for i in 0..self.blocks.len() {
            if !self.blocks[i].in_use && self.blocks[i].size >= size {
                // Extract values BEFORE creating mutable reference
                let addr = self.blocks[i].start;
                let block_size = self.blocks[i].size;
                
                if block_size > size {
                    let remaining = MemoryBlock {
                        start: addr + size,
                        size: block_size - size,
                        in_use: false,
                    };
                    self.blocks[i].size = size;
                    self.blocks.insert(i + 1, remaining);  // ✅ No borrow conflict
                }
                
                self.blocks[i].in_use = true;  // ✅ Direct access, no conflict
                return Some(addr);
            }
        }
        None
    }

    fn free(&mut self, addr: usize) -> bool {
        if let Some(idx) = self.blocks.iter().position(|b| b.start == addr && b.in_use) {
            self.blocks[idx].in_use = false;
            self.coalesce(idx);
            true
        } else {
            false
        }
    }

    fn coalesce(&mut self, idx: usize) {
        if idx + 1 < self.blocks.len() && !self.blocks[idx + 1].in_use {
            let next_size = self.blocks[idx + 1].size;
            self.blocks[idx].size += next_size;
            self.blocks.remove(idx + 1);
        }
        if idx > 0 && !self.blocks[idx - 1].in_use {
            self.blocks[idx - 1].size += self.blocks[idx].size;
            self.blocks.remove(idx);
        }
    }

    fn stats(&self) -> (usize, usize, usize) {
        let used: usize = self.blocks.iter().filter(|b| b.in_use).map(|b| b.size).sum();
        (used, self.memory_size - used, self.blocks.len())
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// ENHANCED Memory Addressing (with AT&T syntax support)
// ═══════════════════════════════════════════════════════════════════════════

#[derive(Debug, Clone)]
enum MemoryAddr {
    Register(String),                          // [reg] or (reg)
    Immediate(u64),                            // [0x1000] or 0x1000
    RegOffset(String, i64),                    // [reg + offset] or offset(reg)
    RegReg(String, String),                    // [reg1 + reg2] or (reg1,reg2)
    RegRegScale(String, String, u8),           // [base + index*scale] or (base,index,scale)
    RegRegScaleDisp(String, String, u8, i64),  // [base + index*scale + disp] or disp(base,index,scale)
    RipRelative(i32),                          // [rip + offset] or offset(%rip)
}

// Strip AT&T syntax prefixes (%, $, #)
fn strip_att_prefix(s: &str) -> String {
    s.trim_start_matches('%').trim_start_matches('$').trim_start_matches('#').to_string()
}

// Parse numbers in various formats
fn parse_number(s: &str) -> Result<u64, String> {
    let s = s.trim();
    if s.starts_with("0x") || s.starts_with("0X") {
        return u64::from_str_radix(&s[2..], 16)
            .map_err(|_| format!("Invalid hex: {}", s));
    }
    if s.starts_with("0b") || s.starts_with("0B") {
        return u64::from_str_radix(&s[2..], 2)
            .map_err(|_| format!("Invalid binary: {}", s));
    }
    s.parse::<u64>().map_err(|_| format!("Invalid number: {}", s))
}

// Check if string is a register
fn is_register(s: &str) -> bool {
    let s = strip_att_prefix(s);
    is_valid_register(&s.to_lowercase())
}

// Parse Intel syntax: [reg], [reg+offset], [reg+reg], [reg+reg*scale], etc.
fn parse_intel_memory(inner: &str) -> Result<MemoryAddr, String> {
    // Try immediate: [0x1000]
    if let Ok(val) = parse_number(inner) {
        return Ok(MemoryAddr::Immediate(val));
    }
    
    // Handle RIP-relative
    if inner.to_lowercase().contains("rip") {
        if let Some(pos) = inner.find('+') {
            let offset = parse_number(inner[pos+1..].trim())? as i32;
            return Ok(MemoryAddr::RipRelative(offset));
        }
        return Ok(MemoryAddr::Register("rip".to_string()));
    }
    
    // Simple register: [rax]
    if !inner.contains('+') && !inner.contains('-') && !inner.contains('*') {
        let reg = strip_att_prefix(inner);
        return Ok(MemoryAddr::Register(reg));
    }
    
    // Complex: [base + index*scale + disp]
    if let Some(plus_pos) = inner.find('+') {
        let left = inner[..plus_pos].trim();
        let right = inner[plus_pos + 1..].trim();
        
        // Check for scale: [reg + reg*scale]
        if right.contains('*') {
            let parts: Vec<&str> = right.split('*').collect();
            if parts.len() == 2 {
                let index = strip_att_prefix(parts[0].trim());
                let scale = parts[1].trim().parse::<u8>()
                    .map_err(|_| format!("Invalid scale: {}", parts[1]))?;
                let base = strip_att_prefix(left);
                
                // Check for displacement after scale
                if parts[1].contains('+') {
                    let scale_parts: Vec<&str> = parts[1].split('+').collect();
                    let scale_val = scale_parts[0].trim().parse::<u8>()
                        .map_err(|_| "Invalid scale")?;
                    let disp = parse_number(scale_parts[1].trim())? as i64;
                    return Ok(MemoryAddr::RegRegScaleDisp(base, index, scale_val, disp));
                }
                
                return Ok(MemoryAddr::RegRegScale(base, index, scale));
            }
        }
        
        // [reg + reg]
        if is_register(right) {
            let base = strip_att_prefix(left);
            let index = strip_att_prefix(right);
            return Ok(MemoryAddr::RegReg(base, index));
        }
        
        // [reg + offset]
        if let Ok(offset) = parse_number(right) {
            let reg = strip_att_prefix(left);
            return Ok(MemoryAddr::RegOffset(reg, offset as i64));
        }
    }
    
    // [reg - offset]
    if let Some(minus_pos) = inner.find('-') {
        let left = inner[..minus_pos].trim();
        let right = inner[minus_pos + 1..].trim();
        if let Ok(offset) = parse_number(right) {
            let reg = strip_att_prefix(left);
            return Ok(MemoryAddr::RegOffset(reg, -(offset as i64)));
        }
    }
    
    Err(format!("Could not parse memory address: [{}]", inner))
}

// Parse AT&T syntax: (%reg), offset(%reg), (%base,%index,scale), etc.
fn parse_att_memory(inner: &str) -> Result<MemoryAddr, String> {
    // Simple: (%reg)
    if !inner.contains(',') && !inner.contains('+') {
        let reg = strip_att_prefix(inner);
        return Ok(MemoryAddr::Register(reg));
    }
    
    // Check for comma (AT&T SIB syntax)
    if inner.contains(',') {
        let parts: Vec<&str> = inner.split(',').collect();
        if parts.len() >= 2 {
            let base = strip_att_prefix(parts[0].trim());
            let index = strip_att_prefix(parts[1].trim());
            
            if parts.len() == 3 {
                // (base, index, scale)
                let scale = parts[2].trim().parse::<u8>()
                    .map_err(|_| "Invalid scale in AT&T syntax")?;
                return Ok(MemoryAddr::RegRegScale(base, index, scale));
            } else {
                // (base, index) - scale = 1
                return Ok(MemoryAddr::RegReg(base, index));
            }
        }
    }
    
    Err(format!("Could not parse AT&T memory: ({})", inner))
}

// Main memory address parser
fn parse_memory_addr(addr_str: &str) -> Result<MemoryAddr, String> {
    let addr_str = addr_str.trim();
    
    // Intel syntax: [...]
    if addr_str.starts_with('[') && addr_str.ends_with(']') {
        let inner = addr_str[1..addr_str.len() - 1].trim();
        return parse_intel_memory(inner);
    }
    
    // AT&T syntax: (...) or offset(...)
    if addr_str.contains('(') && addr_str.ends_with(')') {
        if let Some(paren_pos) = addr_str.find('(') {
            if paren_pos == 0 {
                // Simple: (%reg) or (%base,%index,scale)
                let inner = &addr_str[1..addr_str.len() - 1];
                return parse_att_memory(inner);
            } else {
                // Displacement: offset(%reg) or offset(%base,%index,scale)
                let disp_str = &addr_str[..paren_pos];
                let disp = parse_number(disp_str)? as i64;
                let inner = &addr_str[paren_pos + 1..addr_str.len() - 1];
                
                if inner.contains(',') {
                    // offset(%base,%index,scale)
                    let parts: Vec<&str> = inner.split(',').collect();
                    if parts.len() >= 2 {
                        let base = strip_att_prefix(parts[0].trim());
                        let index = strip_att_prefix(parts[1].trim());
                        let scale = if parts.len() == 3 {
                            parts[2].trim().parse::<u8>()
                                .map_err(|_| "Invalid scale")?
                        } else {
                            1
                        };
                        return Ok(MemoryAddr::RegRegScaleDisp(base, index, scale, disp));
                    }
                } else {
                    // offset(%reg)
                    let reg = strip_att_prefix(inner);
                    return Ok(MemoryAddr::RegOffset(reg, disp));
                }
            }
        }
    }
    
    Err(format!("Invalid memory address format: {}", addr_str))
}


// ═══════════════════════════════════════════════════════════════════════════
// Register Validation (ALL registers from lists)
// ═══════════════════════════════════════════════════════════════════════════

fn is_valid_x86_register(reg: &str) -> bool {
    matches!(reg,
        "rax" | "rbx" | "rcx" | "rdx" | "rsi" | "rdi" | "rsp" | "rbp" |
        "r8" | "r9" | "r10" | "r11" | "r12" | "r13" | "r14" | "r15" |
        "eax" | "ebx" | "ecx" | "edx" | "esi" | "edi" | "esp" | "ebp" |
        "r8d" | "r9d" | "r10d" | "r11d" | "r12d" | "r13d" | "r14d" | "r15d" |
        "ax" | "bx" | "cx" | "dx" | "si" | "di" | "sp" | "bp" |
        "r8w" | "r9w" | "r10w" | "r11w" | "r12w" | "r13w" | "r14w" | "r15w" |
        "al" | "ah" | "bl" | "bh" | "cl" | "ch" | "dl" | "dh" |
        "sil" | "dil" | "bpl" | "spl" |
        "r8b" | "r9b" | "r10b" | "r11b" | "r12b" | "r13b" | "r14b" | "r15b" |
        "xmm0" | "xmm1" | "xmm2" | "xmm3" | "xmm4" | "xmm5" | "xmm6" | "xmm7" |
        "xmm8" | "xmm9" | "xmm10" | "xmm11" | "xmm12" | "xmm13" | "xmm14" | "xmm15" |
        "ymm0" | "ymm1" | "ymm2" | "ymm3" | "ymm4" | "ymm5" | "ymm6" | "ymm7" |
        "ymm8" | "ymm9" | "ymm10" | "ymm11" | "ymm12" | "ymm13" | "ymm14" | "ymm15" |
        "zmm0" | "zmm1" | "zmm2" | "zmm3" | "zmm4" | "zmm5" | "zmm6" | "zmm7" |
        "zmm8" | "zmm9" | "zmm10" | "zmm11" | "zmm12" | "zmm13" | "zmm14" | "zmm15" |
        "st0" | "st1" | "st2" | "st3" | "st4" | "st5" | "st6" | "st7" |
        "mm0" | "mm1" | "mm2" | "mm3" | "mm4" | "mm5" | "mm6" | "mm7" |
        "fpr0" | "fpr1" | "fpr2" | "fpr3" | "fpr4" | "fpr5" | "fpr6" | "fpr7" |
        "rip" | "eip" | "ip" | "rflags" | "eflags" | "flags" |
        "cs" | "ds" | "es" | "fs" | "gs" | "ss" |
        "cr0" | "cr1" | "cr2" | "cr3" | "cr4" |
        "dr0" | "dr1" | "dr2" | "dr3" | "dr4" | "dr5" | "dr6" | "dr7" |
        "gdtr" | "idtr" | "ldtr" | "tr"
    )
}

fn is_valid_arm_register(reg: &str) -> bool {
    if reg.len() >= 2 {
        let prefix = &reg[..1];
        let rest = &reg[1..];
        if let Ok(num) = rest.parse::<u32>() {
            return match prefix {
                "x" => num <= 30, "w" => num <= 30,
                "v" | "q" | "d" | "s" | "h" | "b" => num <= 31,
                _ => false,
            };
        }
    }
    matches!(reg,
        "sp" | "wsp" | "pc" | "lr" | "fp" | "xzr" | "wzr" |
        "pstate" | "cpsr" | "apsr" |
        "sp_el0" | "sp_el1" | "sp_el2" | "sp_el3" |
        "elr_el1" | "elr_el2" | "elr_el3" |
        "spsr_el1" | "spsr_el2" | "spsr_el3"
    )
}

fn is_valid_register(reg: &str) -> bool {
    let lower = reg.to_lowercase();
    is_valid_x86_register(&lower) || is_valid_arm_register(&lower)
}

// ═══════════════════════════════════════════════════════════════════════════
// Instruction Enum (ALL from lists)
// ═══════════════════════════════════════════════════════════════════════════

#[derive(Debug, Clone)]
enum Instr {
    MovRegImm(String, u64), MovRegReg(String, String),
    MovRegMem(String, MemoryAddr), MovMemReg(MemoryAddr, String),
    Movl(String, String), Movq(String, String),
    Lea(String, MemoryAddr),
    
    Add(String, u64), AddRegReg(String, String),
    Sub(String, u64), SubRegReg(String, String),
    Mul(String), Imul(String, String),
    Div(String), Inc(String), Dec(String), Neg(String),
    
    And(String, u64), AndRegReg(String, String),
    Or(String, u64), OrRegReg(String, String),
    Xor(String, u64), XorRegReg(String, String),
    Not(String),
    
    Shl(String, i8), Shr(String, i8), Sal(String, i8), Sar(String, i8),
    Rol(String, i8), Ror(String, i8), Rcl(String, i8), Rcr(String, i8),
    Lsl(String, i8), Lsr(String, i8), Asr(String, i8),
    
    Push(String), Pop(String),
    Cmp(String, u64), CmpRegReg(String, String),
    Test(String, u64), TestRegReg(String, String),
    
    Jmp(String), Je(String), Jne(String), Jz(String), Jnz(String),
    Jg(String), Jge(String), Jl(String), Jle(String),
    Ja(String), Jae(String), Jb(String), Jbe(String),
    
    B(String), Beq(String), Bne(String),
    Bcs(String), Bhs(String), Bcc(String), Blo(String),
    Bmi(String), Bpl(String), Bvs(String), Bvc(String),
    Bhi(String), Bls(String), Bge(String), Blt(String),
    Bgt(String), Ble(String), Bal(String),
    
    Call(String), Bl(String), Ret, Nop,
    Label(String), Global(String),
    Text, Data, Bss, Start,
}

// ═══════════════════════════════════════════════════════════════════════════
// Enhanced Parser with AT&T syntax support
// ═══════════════════════════════════════════════════════════════════════════

fn split_lines(code: &str) -> Vec<&str> {
    code.split('\n')
        .flat_map(|line| line.split(";;"))
        .map(|s| s.trim())
        .filter(|s| !s.is_empty() && !s.starts_with(';') && !s.starts_with('#'))
        .collect()
}

fn parse_operand(op: &str) -> String {
    strip_att_prefix(op.trim_end_matches(','))
}

// ═══════════════════════════════════════════════════════════════════════════
// Parsing Functions
// ═══════════════════════════════════════════════════════════════════════════


fn parse_instructions(code: &str) -> Result<Vec<Instr>, String> {
    let mut instrs = vec![];
    let lines = split_lines(code);
    
    for (line_num, line) in lines.iter().enumerate() {
        let line = line.split(';').next().unwrap_or(line).trim();
        if line.is_empty() { continue; }
        
        // Handle directives
        if line.starts_with('.') {
            match line {
                ".text" => instrs.push(Instr::Text),
                ".data" => instrs.push(Instr::Data),
                ".bss" => instrs.push(Instr::Bss),
                _ if line.starts_with(".global ") => {
                    let name = line[8..].trim().to_string();
                    instrs.push(Instr::Global(name));
                }
                _ => {} // Ignore other directives
            }
            continue;
        }
        
        // Handle labels
        if line.ends_with(':') {
            let label = line.trim_end_matches(':').to_string();
            instrs.push(Instr::Label(label));
            continue;
        }
        
        // Handle _start special label
        if line == "_start" {
            instrs.push(Instr::Start);
            continue;
        }

        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.is_empty() { continue; }

        let op = parts[0].to_lowercase();
        let op_str = op.as_str();

        match op_str {
            "mov" | "movl" | "movq" => {
                if parts.len() < 3 {
                    return Err(format!("Line {}: {} requires 2 operands", line_num + 1, op_str));
                }
                let dst = parts[1].trim_end_matches(',');
                let src = parts[2];

                if dst.starts_with('[') {
                    let mem = parse_memory_addr(dst)?;
                    instrs.push(Instr::MovMemReg(mem, src.to_string()));
                } else if src.starts_with('[') {
                    let mem = parse_memory_addr(src)?;
                    instrs.push(Instr::MovRegMem(dst.to_string(), mem));
                } else if let Ok(imm) = src.parse::<u64>() {
                    instrs.push(Instr::MovRegImm(dst.to_string(), imm));
                } else {
                    if op_str == "movl" {
                        instrs.push(Instr::Movl(dst.to_string(), src.to_string()));
                    } else if op_str == "movq" {
                        instrs.push(Instr::Movq(dst.to_string(), src.to_string()));
                    } else {
                        instrs.push(Instr::MovRegReg(dst.to_string(), src.to_string()));
                    }
                }
            }
            
            "lea" => {
                if parts.len() < 3 {
                    return Err(format!("Line {}: lea requires 2 operands", line_num + 1));
                }
                let dst = parts[1].trim_end_matches(',');
                let src = parts[2];
                let mem = parse_memory_addr(src)?;
                instrs.push(Instr::Lea(dst.to_string(), mem));
            }

            "add" => {
                if parts.len() < 3 {
                    return Err(format!("Line {}: add requires 2 operands", line_num + 1));
                }
                let dst = parts[1].trim_end_matches(',');
                let src = parts[2];
                if let Ok(imm) = src.parse::<u64>() {
                    instrs.push(Instr::Add(dst.to_string(), imm));
                } else {
                    instrs.push(Instr::AddRegReg(dst.to_string(), src.to_string()));
                }
            }
            
            "sub" => {
                if parts.len() < 3 {
                    return Err(format!("Line {}: sub requires 2 operands", line_num + 1));
                }
                let dst = parts[1].trim_end_matches(',');
                let src = parts[2];
                if let Ok(imm) = src.parse::<u64>() {
                    instrs.push(Instr::Sub(dst.to_string(), imm));
                } else {
                    instrs.push(Instr::SubRegReg(dst.to_string(), src.to_string()));
                }
            }
            
            "and" => {
                if parts.len() < 3 {
                    return Err(format!("Line {}: and requires 2 operands", line_num + 1));
                }
                let dst = parts[1].trim_end_matches(',');
                let src = parts[2];
                if let Ok(imm) = src.parse::<u64>() {
                    instrs.push(Instr::And(dst.to_string(), imm));
                } else {
                    instrs.push(Instr::AndRegReg(dst.to_string(), src.to_string()));
                }
            }
            
            "or" => {
                if parts.len() < 3 {
                    return Err(format!("Line {}: or requires 2 operands", line_num + 1));
                }
                let dst = parts[1].trim_end_matches(',');
                let src = parts[2];
                if let Ok(imm) = src.parse::<u64>() {
                    instrs.push(Instr::Or(dst.to_string(), imm));
                } else {
                    instrs.push(Instr::OrRegReg(dst.to_string(), src.to_string()));
                }
            }
            
            "xor" => {
                if parts.len() < 3 {
                    return Err(format!("Line {}: xor requires 2 operands", line_num + 1));
                }
                let dst = parts[1].trim_end_matches(',');
                let src = parts[2];
                if let Ok(imm) = src.parse::<u64>() {
                    instrs.push(Instr::Xor(dst.to_string(), imm));
                } else {
                    instrs.push(Instr::XorRegReg(dst.to_string(), src.to_string()));
                }
            }
            
            "cmp" => {
                if parts.len() < 3 {
                    return Err(format!("Line {}: cmp requires 2 operands", line_num + 1));
                }
                let dst = parts[1].trim_end_matches(',');
                let src = parts[2];
                if let Ok(imm) = src.parse::<u64>() {
                    instrs.push(Instr::Cmp(dst.to_string(), imm));
                } else {
                    instrs.push(Instr::CmpRegReg(dst.to_string(), src.to_string()));
                }
            }
            
            "test" => {
                if parts.len() < 3 {
                    return Err(format!("Line {}: test requires 2 operands", line_num + 1));
                }
                let dst = parts[1].trim_end_matches(',');
                let src = parts[2];
                if let Ok(imm) = src.parse::<u64>() {
                    instrs.push(Instr::Test(dst.to_string(), imm));
                } else {
                    instrs.push(Instr::TestRegReg(dst.to_string(), src.to_string()));
                }
            }
            
            // Shift instructions
            "shl" | "sal" => {
                if parts.len() < 3 {
                    return Err(format!("Line {}: shift requires count", line_num + 1));
                }
                let reg = parts[1].trim_end_matches(',');
                let cnt: i8 = parts[2].parse().map_err(|_| "Invalid shift count")?;
                instrs.push(match op_str {
                    "shl" => Instr::Shl(reg.to_string(), cnt),
                    "sal" => Instr::Sal(reg.to_string(), cnt),
                    _ => unreachable!(),
                });
            }
            
            "shr" => {
                if parts.len() < 3 {
                    return Err(format!("Line {}: shr requires count", line_num + 1));
                }
                let reg = parts[1].trim_end_matches(',');
                let cnt: i8 = parts[2].parse().map_err(|_| "Invalid shift count")?;
                instrs.push(Instr::Shr(reg.to_string(), cnt));
            }
            
            "sar" => {
                if parts.len() < 3 {
                    return Err(format!("Line {}: sar requires count", line_num + 1));
                }
                let reg = parts[1].trim_end_matches(',');
                let cnt: i8 = parts[2].parse().map_err(|_| "Invalid shift count")?;
                instrs.push(Instr::Sar(reg.to_string(), cnt));
            }
            
            "rol" => {
                if parts.len() < 3 {
                    return Err(format!("Line {}: rol requires count", line_num + 1));
                }
                let reg = parts[1].trim_end_matches(',');
                let cnt: i8 = parts[2].parse().map_err(|_| "Invalid rotate count")?;
                instrs.push(Instr::Rol(reg.to_string(), cnt));
            }
            
            "ror" => {
                if parts.len() < 3 {
                    return Err(format!("Line {}: ror requires count", line_num + 1));
                }
                let reg = parts[1].trim_end_matches(',');
                let cnt: i8 = parts[2].parse().map_err(|_| "Invalid rotate count")?;
                instrs.push(Instr::Ror(reg.to_string(), cnt));
            }
            
            "rcl" => {
                if parts.len() < 3 {
                    return Err(format!("Line {}: rcl requires count", line_num + 1));
                }
                let reg = parts[1].trim_end_matches(',');
                let cnt: i8 = parts[2].parse().map_err(|_| "Invalid rotate count")?;
                instrs.push(Instr::Rcl(reg.to_string(), cnt));
            }
            
            "rcr" => {
                if parts.len() < 3 {
                    return Err(format!("Line {}: rcr requires count", line_num + 1));
                }
                let reg = parts[1].trim_end_matches(',');
                let cnt: i8 = parts[2].parse().map_err(|_| "Invalid rotate count")?;
                instrs.push(Instr::Rcr(reg.to_string(), cnt));
            }
            
            // ARM shifts
            "lsl" => {
                if parts.len() < 3 {
                    return Err(format!("Line {}: lsl requires count", line_num + 1));
                }
                let reg = parts[1].trim_end_matches(',');
                let cnt: i8 = parts[2].parse().map_err(|_| "Invalid shift count")?;
                instrs.push(Instr::Lsl(reg.to_string(), cnt));
            }
            
            "lsr" => {
                if parts.len() < 3 {
                    return Err(format!("Line {}: lsr requires count", line_num + 1));
                }
                let reg = parts[1].trim_end_matches(',');
                let cnt: i8 = parts[2].parse().map_err(|_| "Invalid shift count")?;
                instrs.push(Instr::Lsr(reg.to_string(), cnt));
            }
            
            "asr" => {
                if parts.len() < 3 {
                    return Err(format!("Line {}: asr requires count", line_num + 1));
                }
                let reg = parts[1].trim_end_matches(',');
                let cnt: i8 = parts[2].parse().map_err(|_| "Invalid shift count")?;
                instrs.push(Instr::Asr(reg.to_string(), cnt));
            }

            // x86-64 Jumps
            "jmp" => {
                if parts.len() > 1 {
                    instrs.push(Instr::Jmp(parts[1].to_string()));
                }
            }
            "je" | "jz" => {
                if parts.len() > 1 {
                    instrs.push(Instr::Je(parts[1].to_string()));
                }
            }
            "jne" | "jnz" => {
                if parts.len() > 1 {
                    instrs.push(Instr::Jne(parts[1].to_string()));
                }
            }
            "jg" => {
                if parts.len() > 1 {
                    instrs.push(Instr::Jg(parts[1].to_string()));
                }
            }
            "jge" => {
                if parts.len() > 1 {
                    instrs.push(Instr::Jge(parts[1].to_string()));
                }
            }
            "jl" => {
                if parts.len() > 1 {
                    instrs.push(Instr::Jl(parts[1].to_string()));
                }
            }
            "jle" => {
                if parts.len() > 1 {
                    instrs.push(Instr::Jle(parts[1].to_string()));
                }
            }
            "ja" => {
                if parts.len() > 1 {
                    instrs.push(Instr::Ja(parts[1].to_string()));
                }
            }
            "jae" => {
                if parts.len() > 1 {
                    instrs.push(Instr::Jae(parts[1].to_string()));
                }
            }
            "jb" => {
                if parts.len() > 1 {
                    instrs.push(Instr::Jb(parts[1].to_string()));
                }
            }
            "jbe" => {
                if parts.len() > 1 {
                    instrs.push(Instr::Jbe(parts[1].to_string()));
                }
            }
            
            // ARM64 Branches - ALL condition codes from list
            "b" => {
                if parts.len() > 1 {
                    instrs.push(Instr::B(parts[1].to_string()));
                }
            }
            "beq" => {
                if parts.len() > 1 {
                    instrs.push(Instr::Beq(parts[1].to_string()));
                }
            }
            "bne" => {
                if parts.len() > 1 {
                    instrs.push(Instr::Bne(parts[1].to_string()));
                }
            }
            "bcs" | "bhs" => {
                if parts.len() > 1 {
                    instrs.push(Instr::Bcs(parts[1].to_string()));
                }
            }
            "bcc" | "blo" => {
                if parts.len() > 1 {
                    instrs.push(Instr::Bcc(parts[1].to_string()));
                }
            }
            "bmi" => {
                if parts.len() > 1 {
                    instrs.push(Instr::Bmi(parts[1].to_string()));
                }
            }
            "bpl" => {
                if parts.len() > 1 {
                    instrs.push(Instr::Bpl(parts[1].to_string()));
                }
            }
            "bvs" => {
                if parts.len() > 1 {
                    instrs.push(Instr::Bvs(parts[1].to_string()));
                }
            }
            "bvc" => {
                if parts.len() > 1 {
                    instrs.push(Instr::Bvc(parts[1].to_string()));
                }
            }
            "bhi" => {
                if parts.len() > 1 {
                    instrs.push(Instr::Bhi(parts[1].to_string()));
                }
            }
            "bls" => {
                if parts.len() > 1 {
                    instrs.push(Instr::Bls(parts[1].to_string()));
                }
            }
            "bge" => {
                if parts.len() > 1 {
                    instrs.push(Instr::Bge(parts[1].to_string()));
                }
            }
            "blt" => {
                if parts.len() > 1 {
                    instrs.push(Instr::Blt(parts[1].to_string()));
                }
            }
            "bgt" => {
                if parts.len() > 1 {
                    instrs.push(Instr::Bgt(parts[1].to_string()));
                }
            }
            "ble" => {
                if parts.len() > 1 {
                    instrs.push(Instr::Ble(parts[1].to_string()));
                }
            }
            "bal" => {
                if parts.len() > 1 {
                    instrs.push(Instr::Bal(parts[1].to_string()));
                }
            }
            
            "call" => {
                if parts.len() > 1 {
                    instrs.push(Instr::Call(parts[1].to_string()));
                }
            }
            "bl" => {
                if parts.len() > 1 {
                    instrs.push(Instr::Bl(parts[1].to_string()));
                }
            }
            
            "nop" => instrs.push(Instr::Nop),
            "ret" => instrs.push(Instr::Ret),
            
            "inc" => {
                if parts.len() > 1 {
                    instrs.push(Instr::Inc(parts[1].to_string()));
                }
            }
            "dec" => {
                if parts.len() > 1 {
                    instrs.push(Instr::Dec(parts[1].to_string()));
                }
            }
            "neg" => {
                if parts.len() > 1 {
                    instrs.push(Instr::Neg(parts[1].to_string()));
                }
            }
            
            "push" => {
                if parts.len() > 1 {
                    instrs.push(Instr::Push(parts[1].to_string()));
                }
            }
            "pop" => {
                if parts.len() > 1 {
                    instrs.push(Instr::Pop(parts[1].to_string()));
                }
            }
            
            "mul" => {
                if parts.len() > 1 {
                    instrs.push(Instr::Mul(parts[1].to_string()));
                }
            }
            "imul" => {
                if parts.len() >= 3 {
                    let dst = parts[1].trim_end_matches(',');
                    let src = parts[2];
                    instrs.push(Instr::Imul(dst.to_string(), src.to_string()));
                } else if parts.len() > 1 {
                    instrs.push(Instr::Mul(parts[1].to_string()));
                }
            }
            "div" => {
                if parts.len() > 1 {
                    instrs.push(Instr::Div(parts[1].to_string()));
                }
            }
            "not" => {
                if parts.len() > 1 {
                    instrs.push(Instr::Not(parts[1].to_string()));
                }
            }
            
            _ => return Err(format!("Line {}: Unknown instruction: {}", line_num + 1, op)),
        }
    }
    Ok(instrs)
}

// ═══════════════════════════════════════════════════════════════════════════
// Main TigerASM Class
// ═══════════════════════════════════════════════════════════════════════════

#[pyclass(unsendable)]
pub struct TigerASM {
    arch: CpuArch,
    asm_code: String,
    exec_buf: Option<ExecutableBuffer>,
    regs_x86: RegistersX86Complete,
    regs_arm: RegistersARM64Complete,
    memory: Vec<u8>,
    allocator: MemoryAllocator,
}

#[pyclass]
#[derive(Clone, Copy, PartialEq)]
pub enum CpuArch {
    Auto,
    X86_64,
    AArch64,
}

// ═══════════════════════════════════════════════════════════════════════════
// Helper Macros (unchanged from original)
// ═══════════════════════════════════════════════════════════════════════════

macro_rules! gen_single_reg_op {
    ($ops:expr, $reg:expr, $op:tt) => {{
        match $reg.as_str() {
            "rax" => dynasm!($ops ; $op rax),
            "rbx" => dynasm!($ops ; $op rbx),
            "rcx" => dynasm!($ops ; $op rcx),
            "rdx" => dynasm!($ops ; $op rdx),
            "rsi" => dynasm!($ops ; $op rsi),
            "rdi" => dynasm!($ops ; $op rdi),
            "rbp" => dynasm!($ops ; $op rbp),
            "r8"  => dynasm!($ops ; $op r8),
            "r9"  => dynasm!($ops ; $op r9),
            "r10" => dynasm!($ops ; $op r10),
            "r12" => dynasm!($ops ; $op r12),
            "r13" => dynasm!($ops ; $op r13),
            "r14" => dynasm!($ops ; $op r14),
            "r15" => dynasm!($ops ; $op r15),
            _ => return Err(PyValueError::new_err(format!("Unsupported register: {}", $reg))),
        }
    }};
}

macro_rules! gen_reg_reg {
    ($ops:expr, $op:ident, $dst:expr, $src:expr) => {{
        let d = $dst.as_str();
        let s = $src.as_str();
        match (d, s) {
            ("rax", "rax") => dynasm!($ops ; $op rax, rax),
            ("rax", "rbx") => dynasm!($ops ; $op rax, rbx),
            ("rax", "rcx") => dynasm!($ops ; $op rax, rcx),
            ("rax", "rdx") => dynasm!($ops ; $op rax, rdx),
            ("rax", "rsi") => dynasm!($ops ; $op rax, rsi),
            ("rax", "rdi") => dynasm!($ops ; $op rax, rdi),
            ("rax", "rbp") => dynasm!($ops ; $op rax, rbp),
            ("rax", "r8")  => dynasm!($ops ; $op rax, r8),
            ("rax", "r9")  => dynasm!($ops ; $op rax, r9),
            ("rax", "r10") => dynasm!($ops ; $op rax, r10),
            ("rax", "r12") => dynasm!($ops ; $op rax, r12),
            ("rax", "r13") => dynasm!($ops ; $op rax, r13),
            ("rax", "r14") => dynasm!($ops ; $op rax, r14),
            ("rax", "r15") => dynasm!($ops ; $op rax, r15),
            ("rbx", "rax") => dynasm!($ops ; $op rbx, rax),
            ("rbx", "rbx") => dynasm!($ops ; $op rbx, rbx),
            ("rcx", "rax") => dynasm!($ops ; $op rcx, rax),
            ("rcx", "rcx") => dynasm!($ops ; $op rcx, rcx),
            ("rdx", "rax") => dynasm!($ops ; $op rdx, rax),
            ("rdx", "rdx") => dynasm!($ops ; $op rdx, rdx),
            ("rsi", "rax") => dynasm!($ops ; $op rsi, rax),
            ("rdi", "rax") => dynasm!($ops ; $op rdi, rax),
            ("rbp", "rax") => dynasm!($ops ; $op rbp, rax),
            ("r8",  "rax") => dynasm!($ops ; $op r8,  rax),
            ("r9",  "rax") => dynasm!($ops ; $op r9,  rax),
            ("r10", "rax") => dynasm!($ops ; $op r10, rax),
            ("r12", "rax") => dynasm!($ops ; $op r12, rax),
            ("r13", "rax") => dynasm!($ops ; $op r13, rax),
            ("r14", "rax") => dynasm!($ops ; $op r14, rax),
            ("r15", "rax") => dynasm!($ops ; $op r15, rax),
            _ => return Err(PyValueError::new_err(format!(
                "Unsupported reg-reg combination: {} {}, {}", stringify!($op), d, s
            ))),
        }
    }};
}

macro_rules! gen_reg_imm {
    ($ops:expr, $op:ident, $reg:expr, $val:expr) => {{
        let imm = $val as i32;
        match $reg.as_str() {
            "rax" => dynasm!($ops ; $op rax, imm),
            "rbx" => dynasm!($ops ; $op rbx, imm),
            "rcx" => dynasm!($ops ; $op rcx, imm),
            "rdx" => dynasm!($ops ; $op rdx, imm),
            "rsi" => dynasm!($ops ; $op rsi, imm),
            "rdi" => dynasm!($ops ; $op rdi, imm),
            "rbp" => dynasm!($ops ; $op rbp, imm),
            "r8"  => dynasm!($ops ; $op r8,  imm),
            "r9"  => dynasm!($ops ; $op r9,  imm),
            "r10" => dynasm!($ops ; $op r10, imm),
            "r12" => dynasm!($ops ; $op r12, imm),
            "r13" => dynasm!($ops ; $op r13, imm),
            "r14" => dynasm!($ops ; $op r14, imm),
            "r15" => dynasm!($ops ; $op r15, imm),
            _ => return Err(PyValueError::new_err(format!(
                "Unsupported register for {} imm: {}", stringify!($op), $reg
            ))),
        }
    }};
}

macro_rules! gen_shift {
    ($ops:expr, $op:ident, $reg:expr, $count:expr) => {{
        if $count == 1 {
            match $reg.as_str() {
                "rax" => dynasm!($ops ; $op rax, 1),
                "rbx" => dynasm!($ops ; $op rbx, 1),
                "rcx" => dynasm!($ops ; $op rcx, 1),
                "rdx" => dynasm!($ops ; $op rdx, 1),
                "rsi" => dynasm!($ops ; $op rsi, 1),
                "rdi" => dynasm!($ops ; $op rdi, 1),
                "rbp" => dynasm!($ops ; $op rbp, 1),
                "r8"  => dynasm!($ops ; $op r8,  1),
                "r9"  => dynasm!($ops ; $op r9,  1),
                "r10" => dynasm!($ops ; $op r10, 1),
                "r12" => dynasm!($ops ; $op r12, 1),
                "r13" => dynasm!($ops ; $op r13, 1),
                "r14" => dynasm!($ops ; $op r14, 1),
                "r15" => dynasm!($ops ; $op r15, 1),
                _ => return Err(PyValueError::new_err("Shift not supported on register")),
            }
        } else {
            dynasm!($ops ; mov cl, $count as i8);
            match $reg.as_str() {
                "rax" => dynasm!($ops ; $op rax, cl),
                "rbx" => dynasm!($ops ; $op rbx, cl),
                "rcx" => dynasm!($ops ; $op rcx, cl),
                "rdx" => dynasm!($ops ; $op rdx, cl),
                "rsi" => dynasm!($ops ; $op rsi, cl),
                "rdi" => dynasm!($ops ; $op rdi, cl),
                "rbp" => dynasm!($ops ; $op rbp, cl),
                "r8"  => dynasm!($ops ; $op r8, cl),
                "r9"  => dynasm!($ops ; $op r9, cl),
                "r10" => dynasm!($ops ; $op r10, cl),
                "r12" => dynasm!($ops ; $op r12, cl),
                "r13" => dynasm!($ops ; $op r13, cl),
                "r14" => dynasm!($ops ; $op r14, cl),
                "r15" => dynasm!($ops ; $op r15, cl),
                _ => return Err(PyValueError::new_err("Variable shift not supported")),
            }
        }
    }};
}

macro_rules! jump {
    ($ops:expr, $op:ident, $label:expr, $labels:expr) => {{
        if let Some(lbl) = $labels.get($label) {
            dynasm!($ops ; $op => *lbl);
        } else {
            return Err(PyValueError::new_err(format!("Undefined label: {}", $label)));
        }
    }};
}

// ═══════════════════════════════════════════════════════════════════════════
// PyMethods Implementation - COMPLETE API
// ═══════════════════════════════════════════════════════════════════════════

#[pymethods]
impl TigerASM {
    #[new]
    pub fn new() -> Self {
        let mem_size = 10 * 1024 * 1024;
        Self {
            arch: CpuArch::Auto,
            asm_code: String::new(),
            exec_buf: None,
            regs_x86: RegistersX86Complete::default(),
            regs_arm: RegistersARM64Complete::default(),
            memory: vec![0u8; mem_size],
            allocator: MemoryAllocator::new(mem_size),
        }
    }

    pub fn setup(&mut self, arch: Option<&str>) {
        self.arch = match arch {
            Some("x86_64") | Some("x86") | Some("amd64") => CpuArch::X86_64,
            Some("aarch64") | Some("arm64") | Some("arm") => CpuArch::AArch64,
            _ => {
                #[cfg(target_arch = "x86_64")]
                { CpuArch::X86_64 }
                #[cfg(target_arch = "aarch64")]
                { CpuArch::AArch64 }
                #[cfg(not(any(target_arch = "x86_64", target_arch = "aarch64")))]
                { CpuArch::Auto }
            }
        };
    }

    pub fn asm(&mut self, code: &str, file: Option<&PyAny>) -> PyResult<()> {
        self.asm_code.push_str(code);
        self.asm_code.push('\n');
        if let Some(f) = file {
            let bytes: Vec<u8> = f.call_method0("read")?.extract()?;
            let content = String::from_utf8(bytes)
                .map_err(|_| PyValueError::new_err("ASM file not UTF-8"))?;
            self.asm_code.push_str(&content);
            self.asm_code.push('\n');
        }
        Ok(())
    }

    pub fn load_file(&mut self, filename: &str) -> PyResult<()> {
        let mut file = File::open(filename)
            .map_err(|e| PyValueError::new_err(format!("File not found: {}", e)))?;
        let mut content = String::new();
        file.read_to_string(&mut content)
            .map_err(|e| PyValueError::new_err(format!("Failed to read file: {}", e)))?;
        self.asm_code.push_str(&content);
        self.asm_code.push('\n');
        Ok(())
    }

    pub fn load_binary_file(&mut self, filename: &str, address: u64) -> PyResult<usize> {
        let mut file = File::open(filename)
            .map_err(|e| PyValueError::new_err(format!("File not found: {}", e)))?;
        let addr = address as usize;
        if addr >= self.memory.len() {
            return Err(PyValueError::new_err("Address out of bounds"));
        }
        let mut buffer = Vec::new();
        let bytes_read = file.read_to_end(&mut buffer)
            .map_err(|e| PyValueError::new_err(format!("Failed to read file: {}", e)))?;
        if addr + bytes_read > self.memory.len() {
            return Err(PyValueError::new_err("Data exceeds memory bounds"));
        }
        self.memory[addr..addr + bytes_read].copy_from_slice(&buffer);
        Ok(bytes_read)
    }

    pub fn load_bytes_to_memory(&mut self, data: Vec<u8>, address: u64) -> PyResult<()> {
        let addr = address as usize;
        if addr + data.len() > self.memory.len() {
            return Err(PyValueError::new_err("Data exceeds memory bounds"));
        }
        self.memory[addr..addr + data.len()].copy_from_slice(&data);
        Ok(())
    }

    pub fn write_bytes(&mut self, address: u64, data: Vec<u8>) -> PyResult<()> {
        let addr = address as usize;
        if addr + data.len() > self.memory.len() {
            return Err(PyValueError::new_err("Write exceeds memory bounds"));
        }
        self.memory[addr..addr + data.len()].copy_from_slice(&data);
        Ok(())
    }

    pub fn read_bytes(&self, address: u64, size: usize) -> PyResult<Vec<u8>> {
        let addr = address as usize;
        if addr + size > self.memory.len() {
            return Err(PyValueError::new_err("Read exceeds memory bounds"));
        }
        Ok(self.memory[addr..addr + size].to_vec())
    }

    pub fn clear_memory(&mut self, address: Option<u64>, size: Option<usize>) {
        match (address, size) {
            (Some(addr), Some(sz)) => {
                let start = addr as usize;
                let end = (start + sz).min(self.memory.len());
                self.memory[start..end].fill(0);
            }
            _ => {
                self.memory.fill(0);
                self.allocator = MemoryAllocator::new(self.memory.len());
            }
        }
    }

    pub fn save_memory_to_file(&self, filename: &str, address: u64, size: usize) -> PyResult<()> {
        let addr = address as usize;
        if addr + size > self.memory.len() {
            return Err(PyValueError::new_err("Memory range out of bounds"));
        }
        let mut file = File::create(filename)
            .map_err(|e| PyValueError::new_err(format!("Failed to create file: {}", e)))?;
        file.write_all(&self.memory[addr..addr + size])
            .map_err(|e| PyValueError::new_err(format!("Failed to write file: {}", e)))?;
        Ok(())
    }

    pub fn memory_size(&self) -> usize {
        self.memory.len()
    }

    pub fn alloc(&mut self, size: usize) -> u64 {
        if let Some(addr) = self.allocator.allocate(size) {
            let start = addr;
            let end = start + size;
            if end <= self.memory.len() {
                self.memory[start..end].fill(0);
            }
            addr as u64
        } else {
            0
        }
    }

    pub fn free(&mut self, address: u64) -> bool {
        self.allocator.free(address as usize)
    }

    pub fn memory_stats(&self) -> (usize, usize, usize) {
        self.allocator.stats()
    }

    pub fn mov(&mut self, register: &str, value: u64) -> PyResult<()> {
        let reg = register.to_lowercase();
        
        if reg.starts_with('r') || reg.starts_with('e') {
            match reg.as_str() {
                "rax" => self.regs_x86.rax = value,
                "rbx" => self.regs_x86.rbx = value,
                "rcx" => self.regs_x86.rcx = value,
                "rdx" => self.regs_x86.rdx = value,
                "rsi" => self.regs_x86.rsi = value,
                "rdi" => self.regs_x86.rdi = value,
                "rsp" => self.regs_x86.rsp = value,
                "rbp" => self.regs_x86.rbp = value,
                "r8" => self.regs_x86.r8 = value,
                "r9" => self.regs_x86.r9 = value,
                "r10" => self.regs_x86.r10 = value,
                "r11" => self.regs_x86.r11 = value,
                "r12" => self.regs_x86.r12 = value,
                "r13" => self.regs_x86.r13 = value,
                "r14" => self.regs_x86.r14 = value,
                "r15" => self.regs_x86.r15 = value,
                "rip" => self.regs_x86.rip = value,
                "rflags" | "eflags" => self.regs_x86.rflags = value,
                _ => return Err(PyValueError::new_err(format!("Unknown x86 register: {}", register))),
            }
        } else if reg.starts_with('x') || reg.starts_with('w') {
            let num = reg[1..].parse::<usize>().ok();
            if let Some(n) = num {
                if n < 31 {
                    self.regs_arm.x[n] = value;
                    if n == 30 {
                        self.regs_arm.lr = value;
                    }
                    if n == 29 {
                        self.regs_arm.fp = value;
                    }
                    return Ok(());
                }
            }
            match reg.as_str() {
                "sp" => self.regs_arm.sp = value,
                "pc" => self.regs_arm.pc = value,
                "lr" => {
                    self.regs_arm.lr = value;
                    self.regs_arm.x[30] = value;
                }
                "fp" => {
                    self.regs_arm.fp = value;
                    self.regs_arm.x[29] = value;
                }
                _ => return Err(PyValueError::new_err(format!("Unknown ARM register: {}", register))),
            }
        } else {
            return Err(PyValueError::new_err(format!("Unknown register: {}", register)));
        }
        Ok(())
    }

    pub fn get(&self, reg: Option<&str>) -> u64 {
        let reg = reg.unwrap_or("rax").to_lowercase();
        
        if reg.starts_with('r') || reg.starts_with('e') {
            match reg.as_str() {
                "rax" => self.regs_x86.rax,
                "rbx" => self.regs_x86.rbx,
                "rcx" => self.regs_x86.rcx,
                "rdx" => self.regs_x86.rdx,
                "rsi" => self.regs_x86.rsi,
                "rdi" => self.regs_x86.rdi,
                "rsp" => self.regs_x86.rsp,
                "rbp" => self.regs_x86.rbp,
                "r8" => self.regs_x86.r8,
                "r9" => self.regs_x86.r9,
                "r10" => self.regs_x86.r10,
                "r11" => self.regs_x86.r11,
                "r12" => self.regs_x86.r12,
                "r13" => self.regs_x86.r13,
                "r14" => self.regs_x86.r14,
                "r15" => self.regs_x86.r15,
                "rip" => self.regs_x86.rip,
                "rflags" | "eflags" => self.regs_x86.rflags,
                _ => 0,
            }
        } else if reg.starts_with('x') || reg.starts_with('w') {
            let num = reg[1..].parse::<usize>().ok();
            if let Some(n) = num {
                if n < 31 {
                    return self.regs_arm.x[n];
                }
            }
            match reg.as_str() {
                "sp" => self.regs_arm.sp,
                "pc" => self.regs_arm.pc,
                "lr" => self.regs_arm.lr,
                "fp" => self.regs_arm.fp,
                _ => 0,
            }
        } else {
            0
        }
    }

    pub fn write_mem(&mut self, address: u64, value: u64, size: u8) -> PyResult<()> {
        let addr = address as usize;
        if addr + size as usize > self.memory.len() {
            return Err(PyValueError::new_err("Memory address out of bounds"));
        }
        match size {
            1 => self.memory[addr] = value as u8,
            2 => self.memory[addr..addr+2].copy_from_slice(&(value as u16).to_le_bytes()),
            4 => self.memory[addr..addr+4].copy_from_slice(&(value as u32).to_le_bytes()),
            8 => self.memory[addr..addr+8].copy_from_slice(&value.to_le_bytes()),
            _ => return Err(PyValueError::new_err("Invalid size (1,2,4,8 only)")),
        }
        Ok(())
    }

    pub fn read_mem(&self, address: u64, size: u8) -> PyResult<u64> {
        let addr = address as usize;
        if addr + size as usize > self.memory.len() {
            return Err(PyValueError::new_err("Memory address out of bounds"));
        }
        let val = match size {
            1 => self.memory[addr] as u64,
            2 => u16::from_le_bytes([self.memory[addr], self.memory[addr+1]]) as u64,
            4 => u32::from_le_bytes([
                self.memory[addr], self.memory[addr+1],
                self.memory[addr+2], self.memory[addr+3]
            ]) as u64,
            8 => u64::from_le_bytes([
                self.memory[addr], self.memory[addr+1], self.memory[addr+2], self.memory[addr+3],
                self.memory[addr+4], self.memory[addr+5], self.memory[addr+6], self.memory[addr+7],
            ]),
            _ => return Err(PyValueError::new_err("Invalid size")),
        };
        Ok(val)
    }

    pub fn get_memory_ptr(&self) -> u64 {
        self.memory.as_ptr() as u64
    }

    pub fn get_arch(&self) -> String {
        match self.arch {
            CpuArch::X86_64 => "x86_64".to_string(),
            CpuArch::AArch64 => "aarch64".to_string(),
            CpuArch::Auto => "auto".to_string(),
        }
    }

    pub fn execute(&mut self) -> PyResult<()> {
        self.compile_asm()?;
        unsafe {
            #[cfg(target_arch = "x86_64")]
            {
                if let Some(buf) = &self.exec_buf {
                    let f: extern "sysv64" fn() -> u64 = std::mem::transmute(buf.ptr(AssemblyOffset(0)));
                    let ret = f();
                    self.regs_x86.rax = ret;
                }
            }
            #[cfg(target_arch = "aarch64")]
            {
                if let Some(buf) = &self.exec_buf {
                    let f: extern "C" fn() -> u64 = std::mem::transmute(buf.ptr(AssemblyOffset(0)));
                    let ret = f();
                    self.regs_arm.x[0] = ret;
                }
            }
        }
        Ok(())
    }

    pub fn install(&mut self, filename: &str) -> PyResult<()> {
        let buf = self.exec_buf.as_ref()
            .ok_or_else(|| PyValueError::new_err("No executable buffer available"))?;
        let mut file = File::create(filename)
            .map_err(|e| PyValueError::new_err(format!("Failed to create file: {}", e)))?;
        file.write_all(buf.as_ref())
            .map_err(|e| PyValueError::new_err(format!("Failed to write file: {}", e)))?;
        Ok(())
    }

    pub fn clear(&mut self) {
        self.asm_code.clear();
        self.exec_buf = None;
    }

    pub fn dump_regs(&self) -> String {
        match self.arch {
            CpuArch::X86_64 => self.dump_regs_x86(),
            CpuArch::AArch64 => self.dump_regs_arm(),
            CpuArch::Auto => {
                #[cfg(target_arch = "x86_64")]
                return self.dump_regs_x86();
                #[cfg(target_arch = "aarch64")]
                return self.dump_regs_arm();
                #[cfg(not(any(target_arch = "x86_64", target_arch = "aarch64")))]
                return "Architecture not supported".to_string();
            }
        }
    }

    pub fn dump_regs_x86(&self) -> String {
        format!(
            "=== x86-64 Registers ===\n\
             RAX={:016x} RBX={:016x} RCX={:016x} RDX={:016x}\n\
             RSI={:016x} RDI={:016x} RBP={:016x} RSP={:016x}\n\
             R8 ={:016x} R9 ={:016x} R10={:016x} R11={:016x}\n\
             R12={:016x} R13={:016x} R14={:016x} R15={:016x}\n\
             RIP={:016x} RFLAGS={:016x}\n\
             Segment: CS={:04x} DS={:04x} ES={:04x} FS={:04x} GS={:04x} SS={:04x}",
            self.regs_x86.rax, self.regs_x86.rbx, self.regs_x86.rcx, self.regs_x86.rdx,
            self.regs_x86.rsi, self.regs_x86.rdi, self.regs_x86.rbp, self.regs_x86.rsp,
            self.regs_x86.r8, self.regs_x86.r9, self.regs_x86.r10, self.regs_x86.r11,
            self.regs_x86.r12, self.regs_x86.r13, self.regs_x86.r14, self.regs_x86.r15,
            self.regs_x86.rip, self.regs_x86.rflags,
            self.regs_x86.cs, self.regs_x86.ds, self.regs_x86.es,
            self.regs_x86.fs, self.regs_x86.gs, self.regs_x86.ss,
        )
    }

    pub fn dump_regs_arm(&self) -> String {
        let mut output = String::from("=== ARM64 Registers ===\n");
        for i in 0..31 {
            output.push_str(&format!("X{:02}={:016x} ", i, self.regs_arm.x[i]));
            if (i + 1) % 4 == 0 {
                output.push('\n');
            }
        }
        output.push_str(&format!(
            "\nSP ={:016x} PC ={:016x} LR ={:016x} FP ={:016x}\n\
             PSTATE={:016x} CPSR={:08x}",
            self.regs_arm.sp, self.regs_arm.pc, self.regs_arm.lr, self.regs_arm.fp,
            self.regs_arm.pstate, self.regs_arm.cpsr
        ));
        output
    }
    
    /// Validate if a register name is valid for current architecture
    pub fn is_valid_register(&self, register: &str) -> bool {
        is_valid_register(register)
    }
}

// Continued in next part due to length...
// [The compile_asm, compile_x86, compile_arm implementations follow]

// ═══════════════════════════════════════════════════════════════════════════
// Compilation Implementation
// ═══════════════════════════════════════════════════════════════════════════

impl TigerASM {
    fn compile_asm(&mut self) -> PyResult<()> {
        #[cfg(target_arch = "x86_64")]
        return self.compile_x86();
        #[cfg(target_arch = "aarch64")]
        return self.compile_arm();
        #[cfg(not(any(target_arch = "x86_64", target_arch = "aarch64")))]
        Err(PyValueError::new_err("Unsupported architecture"))
    }

    #[cfg(target_arch = "x86_64")]
    fn compile_x86(&mut self) -> PyResult<()> {
        let mut ops = AssemblerX64::new()
            .map_err(|e| PyValueError::new_err(format!("Assembler creation failed: {:?}", e)))?;

        let instrs = parse_instructions(&self.asm_code)
            .map_err(PyValueError::new_err)?;

        let exit_label = ops.new_dynamic_label();

        let mut labels: HashMap<String, dynasmrt::DynamicLabel> = HashMap::new();
        for ins in &instrs {
            if let Instr::Label(name) = ins {
                labels.entry(name.clone()).or_insert_with(|| ops.new_dynamic_label());
            }
        }

        // Prologue
        dynasm!(ops
            ; push rbp
            ; mov rbp, rsp
            ; push rbx
            ; push r12
            ; push r13
            ; push r14
            ; push r15
        );

        // Load registers
        dynasm!(ops
            ; mov rax, QWORD self.regs_x86.rax as _
            ; mov rbx, QWORD self.regs_x86.rbx as _
            ; mov rcx, QWORD self.regs_x86.rcx as _
            ; mov rdx, QWORD self.regs_x86.rdx as _
            ; mov rsi, QWORD self.regs_x86.rsi as _
            ; mov rdi, QWORD self.regs_x86.rdi as _
            ; mov r8,  QWORD self.regs_x86.r8 as _
            ; mov r9,  QWORD self.regs_x86.r9 as _
            ; mov r10, QWORD self.regs_x86.r10 as _
            ; mov r12, QWORD self.regs_x86.r12 as _
            ; mov r13, QWORD self.regs_x86.r13 as _
            ; mov r14, QWORD self.regs_x86.r14 as _
            ; mov r15, QWORD self.regs_x86.r15 as _
        );

        for ins in instrs {
            match ins {
                Instr::Label(name) => {
                    if let Some(l) = labels.get(&name) {
                        dynasm!(ops ; => *l);
                    }
                }
                
                Instr::Text | Instr::Data | Instr::Bss | Instr::Global(_) | Instr::Start => {
                    // Directives - no codegen needed
                }

                Instr::MovRegImm(reg, val) => {
                    let v = val as i64;
                    match reg.as_str() {
                        "rax" => dynasm!(ops ; mov rax, QWORD v),
                        "rbx" => dynasm!(ops ; mov rbx, QWORD v),
                        "rcx" => dynasm!(ops ; mov rcx, QWORD v),
                        "rdx" => dynasm!(ops ; mov rdx, QWORD v),
                        "rsi" => dynasm!(ops ; mov rsi, QWORD v),
                        "rdi" => dynasm!(ops ; mov rdi, QWORD v),
                        "rbp" => dynasm!(ops ; mov rbp, QWORD v),
                        "r8"  => dynasm!(ops ; mov r8,  QWORD v),
                        "r9"  => dynasm!(ops ; mov r9,  QWORD v),
                        "r10" => dynasm!(ops ; mov r10, QWORD v),
                        "r12" => dynasm!(ops ; mov r12, QWORD v),
                        "r13" => dynasm!(ops ; mov r13, QWORD v),
                        "r14" => dynasm!(ops ; mov r14, QWORD v),
                        "r15" => dynasm!(ops ; mov r15, QWORD v),
                        _ => return Err(PyValueError::new_err("Unsupported mov imm register")),
                    }
                }

                Instr::MovRegReg(dst, src) => gen_reg_reg!(ops, mov, dst, src),
                Instr::Movl(dst, src) => gen_reg_reg!(ops, mov, dst, src),
                Instr::Movq(dst, src) => gen_reg_reg!(ops, mov, dst, src),

                Instr::Add(reg, val) => gen_reg_imm!(ops, add, reg, val),
                Instr::AddRegReg(a, b) => gen_reg_reg!(ops, add, a, b),
                Instr::Sub(reg, val) => gen_reg_imm!(ops, sub, reg, val),
                Instr::SubRegReg(a, b) => gen_reg_reg!(ops, sub, a, b),

                Instr::And(reg, val) => gen_reg_imm!(ops, and, reg, val),
                Instr::Or(reg, val) => gen_reg_imm!(ops, or, reg, val),
                Instr::Xor(reg, val) => gen_reg_imm!(ops, xor, reg, val),

                Instr::AndRegReg(a, b) => gen_reg_reg!(ops, and, a, b),
                Instr::OrRegReg(a, b) => gen_reg_reg!(ops, or, a, b),
                Instr::XorRegReg(a, b) => gen_reg_reg!(ops, xor, a, b),

                Instr::Shl(reg, c) => gen_shift!(ops, shl, reg, c),
                Instr::Shr(reg, c) => gen_shift!(ops, shr, reg, c),
                Instr::Sal(reg, c) => gen_shift!(ops, sal, reg, c),
                Instr::Sar(reg, c) => gen_shift!(ops, sar, reg, c),
                Instr::Rol(reg, c) => gen_shift!(ops, rol, reg, c),
                Instr::Ror(reg, c) => gen_shift!(ops, ror, reg, c),
                Instr::Rcl(reg, c) => gen_shift!(ops, rcl, reg, c),
                Instr::Rcr(reg, c) => gen_shift!(ops, rcr, reg, c),

                Instr::Mul(reg) => gen_single_reg_op!(ops, reg, mul),
                Instr::Imul(a, b) => gen_reg_reg!(ops, imul, a, b),
                Instr::Div(reg) => gen_single_reg_op!(ops, reg, div),

                Instr::Inc(reg) => gen_single_reg_op!(ops, reg, inc),
                Instr::Dec(reg) => gen_single_reg_op!(ops, reg, dec),
                Instr::Neg(reg) => gen_single_reg_op!(ops, reg, neg),
                Instr::Not(reg) => gen_single_reg_op!(ops, reg, not),

                Instr::Push(reg) => gen_single_reg_op!(ops, reg, push),
                Instr::Pop(reg) => gen_single_reg_op!(ops, reg, pop),

                Instr::Cmp(reg, val) => gen_reg_imm!(ops, cmp, reg, val),
                Instr::CmpRegReg(a, b) => gen_reg_reg!(ops, cmp, a, b),
                Instr::Test(reg, val) => gen_reg_imm!(ops, test, reg, val),
                Instr::TestRegReg(a, b) => gen_reg_reg!(ops, test, a, b),

                Instr::Jmp(l) | Instr::B(l) => jump!(ops, jmp, &l, labels),
                Instr::Je(l) | Instr::Jz(l) | Instr::Beq(l) => jump!(ops, je, &l, labels),
                Instr::Jne(l) | Instr::Jnz(l) | Instr::Bne(l) => jump!(ops, jne, &l, labels),
                Instr::Jg(l) | Instr::Bgt(l) => jump!(ops, jg, &l, labels),
                Instr::Jge(l) | Instr::Bge(l) => jump!(ops, jge, &l, labels),
                Instr::Jl(l) | Instr::Blt(l) => jump!(ops, jl, &l, labels),
                Instr::Jle(l) | Instr::Ble(l) => jump!(ops, jle, &l, labels),
                Instr::Ja(l) => jump!(ops, ja, &l, labels),
                Instr::Jae(l) => jump!(ops, jae, &l, labels),
                Instr::Jb(l) => jump!(ops, jb, &l, labels),
                Instr::Jbe(l) => jump!(ops, jbe, &l, labels),
                Instr::Call(l) | Instr::Bl(l) => jump!(ops, call, &l, labels),
                
                // ARM condition branches mapped to x86 equivalents
                Instr::Bcs(l) | Instr::Bhs(l) => jump!(ops, jae, &l, labels),
                Instr::Bcc(l) | Instr::Blo(l) => jump!(ops, jb, &l, labels),
                Instr::Bmi(l) => jump!(ops, js, &l, labels),
                Instr::Bpl(l) => jump!(ops, jns, &l, labels),
                Instr::Bvs(l) => jump!(ops, jo, &l, labels),
                Instr::Bvc(l) => jump!(ops, jno, &l, labels),
                Instr::Bhi(l) => jump!(ops, ja, &l, labels),
                Instr::Bls(l) => jump!(ops, jbe, &l, labels),
                Instr::Bal(l) => jump!(ops, jmp, &l, labels),

                Instr::Nop => dynasm!(ops ; nop),
                Instr::Ret => dynasm!(ops ; jmp =>exit_label),

                _ => return Err(PyValueError::new_err("Instruction not implemented for x86-64")),
            }
        }

        // Epilogue
        dynasm!(ops
            ; => exit_label
            ; pop r15
            ; pop r14
            ; pop r13
            ; pop r12
            ; pop rbx
            ; mov rsp, rbp
            ; pop rbp
            ; ret
        );

        self.exec_buf = Some(ops.finalize().map_err(|e| {
            PyValueError::new_err(format!("Assembly failed: {:?}", e))
        })?);

        Ok(())
    }

    #[cfg(target_arch = "aarch64")]
    fn compile_arm(&mut self) -> PyResult<()> {
        let mut ops = AssemblerAArch64::new()
            .map_err(|e| PyValueError::new_err(format!("ARM64 assembler creation failed: {:?}", e)))?;

        let instrs = parse_instructions(&self.asm_code)
            .map_err(PyValueError::new_err)?;

        let exit_label = ops.new_dynamic_label();

        // Collect all labels first
        let mut labels: HashMap<String, dynasmrt::DynamicLabel> = HashMap::new();
        for ins in &instrs {
            if let Instr::Label(name) = ins {
                labels.entry(name.clone()).or_insert_with(|| ops.new_dynamic_label());
            }
        }

        // Prologue - save callee-saved registers and setup frame
        dynasm!(ops
            ; stp x29, x30, [sp, #-16]!  // Save FP and LR
            ; mov x29, sp                 // Set up frame pointer
            ; stp x19, x20, [sp, #-16]!  // Save callee-saved registers
            ; stp x21, x22, [sp, #-16]!
            ; stp x23, x24, [sp, #-16]!
            ; stp x25, x26, [sp, #-16]!
            ; stp x27, x28, [sp, #-16]!
        );

        // Load initial register values
        // Note: In ARM64, we load immediate values using MOV with shifted immediates
        // For full 64-bit values, we might need multiple instructions
        let load_reg_value = |ops: &mut AssemblerAArch64, reg: u8, val: u64| {
            if val <= 0xFFFF {
                // Small immediate - single MOV
                dynasm!(ops ; movz X(reg), val as u32);
            } else if val <= 0xFFFFFFFF {
                // 32-bit value - MOVZ + MOVK
                dynasm!(ops
                    ; movz X(reg), (val & 0xFFFF) as u32
                    ; movk X(reg), ((val >> 16) & 0xFFFF) as u32, lsl 16
                );
            } else {
                // Full 64-bit value - multiple MOVK instructions
                dynasm!(ops
                    ; movz X(reg), (val & 0xFFFF) as u32
                    ; movk X(reg), ((val >> 16) & 0xFFFF) as u32, lsl 16
                    ; movk X(reg), ((val >> 32) & 0xFFFF) as u32, lsl 32
                    ; movk X(reg), ((val >> 48) & 0xFFFF) as u32, lsl 48
                );
            }
        };

        // Load register state
        load_reg_value(&mut ops, 0, self.regs_arm.x[0]);
        load_reg_value(&mut ops, 1, self.regs_arm.x[1]);
        load_reg_value(&mut ops, 2, self.regs_arm.x[2]);
        load_reg_value(&mut ops, 3, self.regs_arm.x[3]);
        load_reg_value(&mut ops, 4, self.regs_arm.x[4]);
        load_reg_value(&mut ops, 5, self.regs_arm.x[5]);
        load_reg_value(&mut ops, 6, self.regs_arm.x[6]);
        load_reg_value(&mut ops, 7, self.regs_arm.x[7]);

        // Process instructions
        for ins in instrs {
            match ins {
                Instr::Label(name) => {
                    if let Some(l) = labels.get(&name) {
                        dynasm!(ops ; => *l);
                    }
                }
                
                Instr::Text | Instr::Data | Instr::Bss | Instr::Global(_) | Instr::Start => {
                    // Directives - no codegen needed
                }

                Instr::MovRegImm(reg, val) => {
                    let reg_num = parse_arm_reg_num(&reg)?;
                    load_reg_value(&mut ops, reg_num, val);
                }

                Instr::MovRegReg(dst, src) | Instr::Movl(dst, src) | Instr::Movq(dst, src) => {
                    let dst_num = parse_arm_reg_num(&dst)?;
                    let src_num = parse_arm_reg_num(&src)?;
                    dynasm!(ops ; mov X(dst_num), X(src_num));
                }

                Instr::Add(reg, val) => {
                    let reg_num = parse_arm_reg_num(&reg)?;
                    if val <= 0xFFF {
                        // Immediate fits in 12 bits
                        dynasm!(ops ; add X(reg_num), X(reg_num), val as u32);
                    } else {
                        // Load immediate to temp register and add
                        load_reg_value(&mut ops, 16, val); // Use x16 as temp
                        dynasm!(ops ; add X(reg_num), X(reg_num), x16);
                    }
                }

                Instr::AddRegReg(dst, src) => {
                    let dst_num = parse_arm_reg_num(&dst)?;
                    let src_num = parse_arm_reg_num(&src)?;
                    dynasm!(ops ; add X(dst_num), X(dst_num), X(src_num));
                }

                Instr::Sub(reg, val) => {
                    let reg_num = parse_arm_reg_num(&reg)?;
                    if val <= 0xFFF {
                        dynasm!(ops ; sub X(reg_num), X(reg_num), val as u32);
                    } else {
                        load_reg_value(&mut ops, 16, val);
                        dynasm!(ops ; sub X(reg_num), X(reg_num), x16);
                    }
                }

                Instr::SubRegReg(dst, src) => {
                    let dst_num = parse_arm_reg_num(&dst)?;
                    let src_num = parse_arm_reg_num(&src)?;
                    dynasm!(ops ; sub X(dst_num), X(dst_num), X(src_num));
                }

                Instr::Mul(src) => {
                    // ARM64: MUL dst, src1, src2 (dst = src1 * src2)
                    // Emulate x86 MUL: RAX = RAX * src, RDX = high bits
                    let src_num = parse_arm_reg_num(&src)?;
                    dynasm!(ops
                        ; mul x0, x0, X(src_num)   // x0 (rax) = x0 * src
                        ; umulh x2, x0, X(src_num) // x2 (rdx) = high 64 bits
                    );
                }

                Instr::Imul(dst, src) => {
                    let dst_num = parse_arm_reg_num(&dst)?;
                    let src_num = parse_arm_reg_num(&src)?;
                    dynasm!(ops ; mul X(dst_num), X(dst_num), X(src_num));
                }

                Instr::Div(src) => {
                    // ARM64: UDIV dst, src1, src2 (unsigned division)
                    let src_num = parse_arm_reg_num(&src)?;
                    dynasm!(ops
                        ; udiv x0, x0, X(src_num)  // x0 = x0 / src (quotient)
                        ; msub x2, x0, X(src_num), x0 // x2 = remainder
                    );
                }

                Instr::And(reg, val) => {
                    let reg_num = parse_arm_reg_num(&reg)?;
                    load_reg_value(&mut ops, 16, val);
                    dynasm!(ops ; and X(reg_num), X(reg_num), x16);
                }

                Instr::AndRegReg(dst, src) => {
                    let dst_num = parse_arm_reg_num(&dst)?;
                    let src_num = parse_arm_reg_num(&src)?;
                    dynasm!(ops ; and X(dst_num), X(dst_num), X(src_num));
                }

                Instr::Or(reg, val) => {
                    let reg_num = parse_arm_reg_num(&reg)?;
                    load_reg_value(&mut ops, 16, val);
                    dynasm!(ops ; orr X(reg_num), X(reg_num), x16);
                }

                Instr::OrRegReg(dst, src) => {
                    let dst_num = parse_arm_reg_num(&dst)?;
                    let src_num = parse_arm_reg_num(&src)?;
                    dynasm!(ops ; orr X(dst_num), X(dst_num), X(src_num));
                }

                Instr::Xor(reg, val) => {
                    let reg_num = parse_arm_reg_num(&reg)?;
                    load_reg_value(&mut ops, 16, val);
                    dynasm!(ops ; eor X(reg_num), X(reg_num), x16);
                }

                Instr::XorRegReg(dst, src) => {
                    let dst_num = parse_arm_reg_num(&dst)?;
                    let src_num = parse_arm_reg_num(&src)?;
                    dynasm!(ops ; eor X(dst_num), X(dst_num), X(src_num));
                }

                Instr::Not(reg) => {
                    let reg_num = parse_arm_reg_num(&reg)?;
                    dynasm!(ops ; mvn X(reg_num), X(reg_num));
                }

                Instr::Neg(reg) => {
                    let reg_num = parse_arm_reg_num(&reg)?;
                    dynasm!(ops ; neg X(reg_num), X(reg_num));
                }

                Instr::Lsl(reg, count) => {
                    let reg_num = parse_arm_reg_num(&reg)?;
                    dynasm!(ops ; lsl X(reg_num), X(reg_num), count as u32);
                }

                Instr::Lsr(reg, count) => {
                    let reg_num = parse_arm_reg_num(&reg)?;
                    dynasm!(ops ; lsr X(reg_num), X(reg_num), count as u32);
                }

                Instr::Asr(reg, count) => {
                    let reg_num = parse_arm_reg_num(&reg)?;
                    dynasm!(ops ; asr X(reg_num), X(reg_num), count as u32);
                }

                // x86 shifts mapped to ARM equivalents
                Instr::Shl(reg, count) | Instr::Sal(reg, count) => {
                    let reg_num = parse_arm_reg_num(&reg)?;
                    dynasm!(ops ; lsl X(reg_num), X(reg_num), count as u32);
                }

                Instr::Shr(reg, count) => {
                    let reg_num = parse_arm_reg_num(&reg)?;
                    dynasm!(ops ; lsr X(reg_num), X(reg_num), count as u32);
                }

                Instr::Sar(reg, count) => {
                    let reg_num = parse_arm_reg_num(&reg)?;
                    dynasm!(ops ; asr X(reg_num), X(reg_num), count as u32);
                }

                Instr::Rol(reg, count) | Instr::Ror(reg, count) |
                Instr::Rcl(reg, count) | Instr::Rcr(reg, count) => {
                    // ARM64 has ROR, can emulate ROL as ROR with (64-count)
                    let reg_num = parse_arm_reg_num(&reg)?;
                    match ins {
                        Instr::Ror(_, _) => {
                            dynasm!(ops ; ror X(reg_num), X(reg_num), count as u32);
                        }
                        Instr::Rol(_, _) => {
                            let inv_count = 64 - count;
                            dynasm!(ops ; ror X(reg_num), X(reg_num), inv_count as u32);
                        }
                        _ => {
                            // RCL/RCR require carry flag - complex, skip for now
                            return Err(PyValueError::new_err("RCL/RCR not yet implemented for ARM64"));
                        }
                    }
                }

                Instr::Inc(reg) => {
                    let reg_num = parse_arm_reg_num(&reg)?;
                    dynasm!(ops ; add X(reg_num), X(reg_num), 1);
                }

                Instr::Dec(reg) => {
                    let reg_num = parse_arm_reg_num(&reg)?;
                    dynasm!(ops ; sub X(reg_num), X(reg_num), 1);
                }

                Instr::Cmp(reg, val) => {
                    let reg_num = parse_arm_reg_num(&reg)?;
                    if val <= 0xFFF {
                        dynasm!(ops ; cmp X(reg_num), val as u32);
                    } else {
                        load_reg_value(&mut ops, 16, val);
                        dynasm!(ops ; cmp X(reg_num), x16);
                    }
                }

                Instr::CmpRegReg(dst, src) => {
                    let dst_num = parse_arm_reg_num(&dst)?;
                    let src_num = parse_arm_reg_num(&src)?;
                    dynasm!(ops ; cmp X(dst_num), X(src_num));
                }

                Instr::Test(reg, val) => {
                    let reg_num = parse_arm_reg_num(&reg)?;
                    load_reg_value(&mut ops, 16, val);
                    dynasm!(ops ; tst X(reg_num), x16);
                }

                Instr::TestRegReg(dst, src) => {
                    let dst_num = parse_arm_reg_num(&dst)?;
                    let src_num = parse_arm_reg_num(&src)?;
                    dynasm!(ops ; tst X(dst_num), X(src_num));
                }

                // Branches - ARM64 native
                Instr::B(l) | Instr::Jmp(l) | Instr::Bal(l) => {
                    if let Some(lbl) = labels.get(&l) {
                        dynasm!(ops ; b => *lbl);
                    } else {
                        return Err(PyValueError::new_err(format!("Undefined label: {}", l)));
                    }
                }

                Instr::Beq(l) | Instr::Je(l) | Instr::Jz(l) => {
                    if let Some(lbl) = labels.get(&l) {
                        dynasm!(ops ; b.eq => *lbl);
                    } else {
                        return Err(PyValueError::new_err(format!("Undefined label: {}", l)));
                    }
                }

                Instr::Bne(l) | Instr::Jne(l) | Instr::Jnz(l) => {
                    if let Some(lbl) = labels.get(&l) {
                        dynasm!(ops ; b.ne => *lbl);
                    } else {
                        return Err(PyValueError::new_err(format!("Undefined label: {}", l)));
                    }
                }

                Instr::Bgt(l) | Instr::Jg(l) => {
                    if let Some(lbl) = labels.get(&l) {
                        dynasm!(ops ; b.gt => *lbl);
                    } else {
                        return Err(PyValueError::new_err(format!("Undefined label: {}", l)));
                    }
                }

                Instr::Bge(l) | Instr::Jge(l) => {
                    if let Some(lbl) = labels.get(&l) {
                        dynasm!(ops ; b.ge => *lbl);
                    } else {
                        return Err(PyValueError::new_err(format!("Undefined label: {}", l)));
                    }
                }

                Instr::Blt(l) | Instr::Jl(l) => {
                    if let Some(lbl) = labels.get(&l) {
                        dynasm!(ops ; b.lt => *lbl);
                    } else {
                        return Err(PyValueError::new_err(format!("Undefined label: {}", l)));
                    }
                }

                Instr::Ble(l) | Instr::Jle(l) => {
                    if let Some(lbl) = labels.get(&l) {
                        dynasm!(ops ; b.le => *lbl);
                    } else {
                        return Err(PyValueError::new_err(format!("Undefined label: {}", l)));
                    }
                }

                Instr::Bcs(l) | Instr::Bhs(l) | Instr::Jae(l) => {
                    if let Some(lbl) = labels.get(&l) {
                        dynasm!(ops ; b.cs => *lbl);
                    } else {
                        return Err(PyValueError::new_err(format!("Undefined label: {}", l)));
                    }
                }

                Instr::Bcc(l) | Instr::Blo(l) | Instr::Jb(l) => {
                    if let Some(lbl) = labels.get(&l) {
                        dynasm!(ops ; b.cc => *lbl);
                    } else {
                        return Err(PyValueError::new_err(format!("Undefined label: {}", l)));
                    }
                }

                Instr::Bmi(l) => {
                    if let Some(lbl) = labels.get(&l) {
                        dynasm!(ops ; b.mi => *lbl);
                    } else {
                        return Err(PyValueError::new_err(format!("Undefined label: {}", l)));
                    }
                }

                Instr::Bpl(l) => {
                    if let Some(lbl) = labels.get(&l) {
                        dynasm!(ops ; b.pl => *lbl);
                    } else {
                        return Err(PyValueError::new_err(format!("Undefined label: {}", l)));
                    }
                }

                Instr::Bvs(l) => {
                    if let Some(lbl) = labels.get(&l) {
                        dynasm!(ops ; b.vs => *lbl);
                    } else {
                        return Err(PyValueError::new_err(format!("Undefined label: {}", l)));
                    }
                }

                Instr::Bvc(l) => {
                    if let Some(lbl) = labels.get(&l) {
                        dynasm!(ops ; b.vc => *lbl);
                    } else {
                        return Err(PyValueError::new_err(format!("Undefined label: {}", l)));
                    }
                }

                Instr::Bhi(l) | Instr::Ja(l) => {
                    if let Some(lbl) = labels.get(&l) {
                        dynasm!(ops ; b.hi => *lbl);
                    } else {
                        return Err(PyValueError::new_err(format!("Undefined label: {}", l)));
                    }
                }

                Instr::Bls(l) | Instr::Jbe(l) => {
                    if let Some(lbl) = labels.get(&l) {
                        dynasm!(ops ; b.ls => *lbl);
                    } else {
                        return Err(PyValueError::new_err(format!("Undefined label: {}", l)));
                    }
                }

                Instr::Bl(l) | Instr::Call(l) => {
                    if let Some(lbl) = labels.get(&l) {
                        dynasm!(ops ; bl => *lbl);
                    } else {
                        return Err(PyValueError::new_err(format!("Undefined label: {}", l)));
                    }
                }

                Instr::Ret => {
                    dynasm!(ops ; b => exit_label);
                }

                Instr::Nop => {
                    dynasm!(ops ; nop);
                }

                Instr::Push(_) | Instr::Pop(_) => {
                    // ARM64 doesn't have push/pop instructions
                    // Use STR/LDR with pre/post-index addressing
                    return Err(PyValueError::new_err(
                        "PUSH/POP not directly supported on ARM64 - use STR/LDR with stack operations"
                    ));
                }

                Instr::MovRegMem(_, _) | Instr::MovMemReg(_, _) | Instr::Lea(_, _) => {
                    // Memory operations would require proper addressing mode handling
                    return Err(PyValueError::new_err(
                        "Memory operations not yet fully implemented for ARM64"
                    ));
                }
            }
        }

        // Epilogue - restore registers and return
        dynasm!(ops
            ; => exit_label
            ; ldp x27, x28, [sp], #16   // Restore callee-saved registers
            ; ldp x25, x26, [sp], #16
            ; ldp x23, x24, [sp], #16
            ; ldp x21, x22, [sp], #16
            ; ldp x19, x20, [sp], #16
            ; ldp x29, x30, [sp], #16   // Restore FP and LR
            ; ret                        // Return (branches to address in x30/LR)
        );

        self.exec_buf = Some(ops.finalize().map_err(|e| {
            PyValueError::new_err(format!("ARM64 assembly failed: {:?}", e))
        })?);

        Ok(())
    }
}

// Helper function to parse ARM register numbers
fn parse_arm_reg_num(reg: &str) -> PyResult<u8> {
    let reg_lower = reg.to_lowercase();
    
    // Handle numbered registers (x0-x30, w0-w30)
    if reg_lower.starts_with('x') || reg_lower.starts_with('w') {
        if let Some(num_str) = reg_lower.strip_prefix('x').or_else(|| reg_lower.strip_prefix('w')) {
            if let Ok(num) = num_str.parse::<u8>() {
                if num <= 30 {
                    return Ok(num);
                }
            }
        }
    }
    
    // Handle special registers
    match reg_lower.as_str() {
        "sp" | "wsp" => Ok(31),  // SP is register 31
        "xzr" | "wzr" => Ok(31), // Zero register shares encoding with SP
        "lr" => Ok(30),          // Link register is x30
        "fp" => Ok(29),          // Frame pointer is x29
        _ => Err(PyValueError::new_err(format!("Unknown ARM register: {}", reg))),
    }
}

#[pyfunction]
fn version() -> &'static str {
    "0.1.0"
}

#[pymodule]
fn tigerasm(_py: Python, m: &PyModule) -> PyResult<()> {
    m.add_class::<TigerASM>()?;
    m.add_class::<CpuArch>()?;
    m.add_function(wrap_pyfunction!(version, m)?)?;
    Ok(())
}
