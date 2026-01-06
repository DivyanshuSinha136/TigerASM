# Security Policy

## Supported Versions

The following versions of TigerASM are currently supported with security updates:

| Version | Supported          |
| ------- | ------------------ |
| 0.1.0   | :white_check_mark: |

**Note**: As TigerASM is a runtime assembler that generates and executes machine code at runtime, security is critical. We strongly recommend using the latest stable version to ensure you have all security patches.

## Reporting a Vulnerability

We take security vulnerabilities in TigerASM seriously. Given that TigerASM generates and executes native machine code at runtime, potential vulnerabilities could have significant security implications.

### Where to Report

**Please do NOT report security vulnerabilities through public GitHub issues.**

Instead, please report security vulnerabilities by emailing: **[security@tigerasm.dev]** (or your preferred security contact email)

Alternatively, you can use GitHub's Security Advisory feature:
1. Navigate to the TigerASM repository
2. Click on the "Security" tab
3. Click "Report a vulnerability"

### What to Include

When reporting a vulnerability, please include:

- **Description**: Clear description of the vulnerability
- **Impact**: Potential security impact (code execution, memory corruption, sandbox escape, etc.)
- **Reproduction**: Step-by-step instructions to reproduce the issue
- **Environment**: Python version, TigerASM version, OS, and architecture (x86_64, ARM, etc.)
- **Proof of Concept**: Code snippet demonstrating the vulnerability (if applicable)
- **Suggested Fix**: If you have ideas on how to fix it (optional)

### Response Timeline

- **Initial Response**: Within 48 hours of submission
- **Status Update**: Weekly updates on investigation progress
- **Resolution Timeline**: We aim to patch critical vulnerabilities within 7-14 days

### What to Expect

**If the vulnerability is accepted:**
- We will work on a fix and keep you updated on progress
- You will be credited in the security advisory (unless you prefer to remain anonymous)
- We will coordinate disclosure timing with you
- A CVE may be requested for significant vulnerabilities
- Security patch will be released across supported versions

**If the vulnerability is declined:**
- We will provide a detailed explanation of why it's not considered a security issue
- We may suggest it as a regular bug report or feature request if appropriate

## Security Considerations for Users

TigerASM operates at a low level with direct memory access and code execution. Users should be aware of:

1. **Untrusted Code**: Never assemble and execute untrusted assembly code
2. **Memory Safety**: TigerASM bypasses Python's memory safety - bugs can cause crashes or undefined behavior
3. **Sandboxing**: Consider running TigerASM code in isolated environments (containers, VMs)
4. **Input Validation**: Always validate any external input used in assembly generation
5. **Privilege Level**: Run TigerASM with minimal required privileges

## Scope

The following are **in scope** for security vulnerabilities:

- Memory corruption vulnerabilities (buffer overflows, use-after-free, etc.)
- Code injection vulnerabilities
- Sandbox escape scenarios
- Privilege escalation
- Denial of service through resource exhaustion
- Unsafe defaults that could lead to security issues

The following are **out of scope**:

- Vulnerabilities in third-party dependencies (report to respective projects)
- Social engineering attacks
- Issues requiring physical access to the system
- Bugs that crash only the user's own process without security implications

## Security Updates

Security updates will be published:
- In the repository's Security Advisories
- In release notes with clear **[![SECURITY](https://github.com/DivyanshuSinha136/TigerASM/actions/workflows/codeql.yml/badge.svg)](https://github.com/DivyanshuSinha136/TigerASM/actions/workflows/codeql.yml)** tags
- Via GitHub security notifications to repository watchers

Thank you for helping keep TigerASM and its users safe!
