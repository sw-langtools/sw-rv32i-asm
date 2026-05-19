# sw-rv32i-asm

RV32I assembler.

## Status

MVP work starts with a deliberately small subset for hello-world demos:

- `addi rd, rs1, imm`
- `sb rs2, offset(rs1)`
- `ebreak`

The assembler emits bytes through `sw-rv32i-isa`; it does not own RV32I bit
packing.
