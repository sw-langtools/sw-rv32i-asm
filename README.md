# sw-rv32i-asm

RV32I assembler.

## Status

MVP work starts with a deliberately small subset for hello-world and MMIO demos:

- `addi rd, rs1, imm`
- `li rd, imm`, expanding to `addi` or `lui`/`addi`
- `sb rs2, offset(rs1)`
- `sw rs2, offset(rs1)`
- `ebreak`

The assembler emits bytes through `sw-rv32i-isa`; it does not own RV32I bit
packing.

`assemble_with_profile` validates expanded instructions against the selected
ISA profile, so RV32E rejects high-register operands even when they appear in
pseudo-instructions.
