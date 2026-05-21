//! Minimal RV32I assembler for the hello-world MVP.

use std::fmt;
use std::str::FromStr;

use sw_rv32i_isa::{ImmOp, Instruction, IsaProfile, LoadWidth, Reg, StoreWidth, encode_word};

pub fn assemble(source: &str) -> Result<Vec<u8>, AsmError> {
    assemble_with_profile(source, IsaProfile::RV32I)
}

pub fn assemble_with_profile(source: &str, profile: IsaProfile) -> Result<Vec<u8>, AsmError> {
    let mut bytes = Vec::new();
    for (line_index, raw_line) in source.lines().enumerate() {
        let line_no = line_index + 1;
        let line = strip_comment(raw_line).trim();
        if line.is_empty() {
            continue;
        }
        let insns = parse_instruction(line).map_err(|kind| AsmError { line_no, kind })?;
        for insn in insns {
            profile.validate_instruction(insn).map_err(|_| AsmError {
                line_no,
                kind: AsmErrorKind::ProfileViolation,
            })?;
            let word = encode_word(insn).map_err(|_| AsmError {
                line_no,
                kind: AsmErrorKind::InvalidOperands,
            })?;
            bytes.extend_from_slice(&word.to_le_bytes());
        }
    }
    Ok(bytes)
}

fn parse_instruction(line: &str) -> Result<Vec<Instruction>, AsmErrorKind> {
    let (mnemonic, rest) = split_mnemonic(line)?;
    match mnemonic {
        "addi" => Ok(vec![parse_addi(rest)?]),
        "li" => parse_li(rest),
        "lw" => Ok(vec![parse_load(rest, LoadWidth::Word)?]),
        "sb" => Ok(vec![parse_store(rest, StoreWidth::Byte)?]),
        "sw" => Ok(vec![parse_store(rest, StoreWidth::Word)?]),
        "ebreak" if rest.trim().is_empty() => Ok(vec![Instruction::Ebreak]),
        "ebreak" => Err(AsmErrorKind::WrongOperandCount),
        _ => Err(AsmErrorKind::UnsupportedMnemonic),
    }
}

fn parse_addi(rest: &str) -> Result<Instruction, AsmErrorKind> {
    let operands = split_operands(rest);
    if operands.len() != 3 {
        return Err(AsmErrorKind::WrongOperandCount);
    }
    Ok(Instruction::OpImm {
        op: ImmOp::Addi,
        rd: parse_reg(operands[0])?,
        rs1: parse_reg(operands[1])?,
        imm: parse_i32_literal(operands[2])?,
    })
}

fn parse_li(rest: &str) -> Result<Vec<Instruction>, AsmErrorKind> {
    let operands = split_operands(rest);
    if operands.len() != 2 {
        return Err(AsmErrorKind::WrongOperandCount);
    }
    let rd = parse_reg(operands[0])?;
    let value = parse_u32_literal(operands[1])?;
    Ok(load_const(rd, value))
}

fn load_const(rd: Reg, value: u32) -> Vec<Instruction> {
    if fits_signed_12(value as i32) {
        return vec![Instruction::OpImm {
            op: ImmOp::Addi,
            rd,
            rs1: Reg::X0,
            imm: value as i32,
        }];
    }

    let hi = value.wrapping_add(0x800) & 0xffff_f000;
    let lo = value.wrapping_sub(hi) as i32;
    let mut insns = vec![Instruction::Lui { rd, imm: hi as i32 }];
    if lo != 0 {
        insns.push(Instruction::OpImm {
            op: ImmOp::Addi,
            rd,
            rs1: rd,
            imm: lo,
        });
    }
    insns
}

fn fits_signed_12(value: i32) -> bool {
    (-2048..=2047).contains(&value)
}

fn parse_store(rest: &str, width: StoreWidth) -> Result<Instruction, AsmErrorKind> {
    let operands = split_operands(rest);
    if operands.len() != 2 {
        return Err(AsmErrorKind::WrongOperandCount);
    }
    let (offset, base) = parse_offset_base(operands[1])?;
    Ok(Instruction::Store {
        width,
        rs1: base,
        rs2: parse_reg(operands[0])?,
        offset,
    })
}

fn parse_load(rest: &str, width: LoadWidth) -> Result<Instruction, AsmErrorKind> {
    let operands = split_operands(rest);
    if operands.len() != 2 {
        return Err(AsmErrorKind::WrongOperandCount);
    }
    let (offset, base) = parse_offset_base(operands[1])?;
    Ok(Instruction::Load {
        width,
        rd: parse_reg(operands[0])?,
        rs1: base,
        offset,
    })
}

fn split_mnemonic(line: &str) -> Result<(&str, &str), AsmErrorKind> {
    let line = line.trim_start();
    if line.is_empty() {
        return Err(AsmErrorKind::EmptyInstruction);
    }
    let split = line.find(char::is_whitespace).unwrap_or(line.len());
    Ok((&line[..split], &line[split..]))
}

fn split_operands(rest: &str) -> Vec<&str> {
    rest.split(',')
        .map(str::trim)
        .filter(|operand| !operand.is_empty())
        .collect()
}

fn parse_offset_base(text: &str) -> Result<(i32, Reg), AsmErrorKind> {
    let open = text.find('(').ok_or(AsmErrorKind::InvalidMemoryOperand)?;
    let close = text.find(')').ok_or(AsmErrorKind::InvalidMemoryOperand)?;
    if close != text.len() - 1 || close < open {
        return Err(AsmErrorKind::InvalidMemoryOperand);
    }
    let offset = parse_i32_literal(text[..open].trim())?;
    let base = parse_reg(text[open + 1..close].trim())?;
    Ok((offset, base))
}

fn parse_reg(text: &str) -> Result<Reg, AsmErrorKind> {
    Reg::from_str(text).map_err(|_| AsmErrorKind::InvalidRegister)
}

fn parse_i32_literal(text: &str) -> Result<i32, AsmErrorKind> {
    Ok(parse_u32_literal(text)? as i32)
}

fn parse_u32_literal(text: &str) -> Result<u32, AsmErrorKind> {
    let text = text.trim();
    if text.is_empty() {
        return Err(AsmErrorKind::InvalidLiteral);
    }
    let (negative, digits) = text
        .strip_prefix('-')
        .map_or((false, text), |rest| (true, rest));
    let value = if let Some(hex) = digits.strip_prefix("0x") {
        u32::from_str_radix(hex, 16).map_err(|_| AsmErrorKind::InvalidLiteral)?
    } else {
        digits
            .parse::<u32>()
            .map_err(|_| AsmErrorKind::InvalidLiteral)?
    };
    if negative {
        Ok((0u32).wrapping_sub(value))
    } else {
        Ok(value)
    }
}

fn strip_comment(line: &str) -> &str {
    let hash = line.find('#');
    let semi = line.find(';');
    match (hash, semi) {
        (Some(a), Some(b)) => &line[..a.min(b)],
        (Some(index), None) | (None, Some(index)) => &line[..index],
        (None, None) => line,
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AsmError {
    pub line_no: usize,
    pub kind: AsmErrorKind,
}

impl fmt::Display for AsmError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "line {}: {}", self.line_no, self.kind)
    }
}

impl std::error::Error for AsmError {}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AsmErrorKind {
    EmptyInstruction,
    UnsupportedMnemonic,
    WrongOperandCount,
    InvalidRegister,
    InvalidLiteral,
    InvalidMemoryOperand,
    InvalidOperands,
    ProfileViolation,
}

impl fmt::Display for AsmErrorKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let message = match self {
            AsmErrorKind::EmptyInstruction => "empty instruction",
            AsmErrorKind::UnsupportedMnemonic => "unsupported mnemonic",
            AsmErrorKind::WrongOperandCount => "wrong operand count",
            AsmErrorKind::InvalidRegister => "invalid register",
            AsmErrorKind::InvalidLiteral => "invalid literal",
            AsmErrorKind::InvalidMemoryOperand => "invalid memory operand",
            AsmErrorKind::InvalidOperands => "invalid operands",
            AsmErrorKind::ProfileViolation => "instruction is not legal for selected ISA profile",
        };
        f.write_str(message)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sw_rv32i_isa::decode_word;

    const HELLO_SOURCE: &str = r#"
        # Write "hello\n" to bytes at 0x100.
        addi x1, x0, 0x100
        addi t0, zero, 0x68
        sb t0, 0(x1)
        addi t0, zero, 0x65
        sb t0, 1(x1)
        addi t0, zero, 0x6c
        sb t0, 2(x1)
        sb t0, 3(x1)
        addi t0, zero, 0x6f
        sb t0, 4(x1)
        addi t0, zero, 0x0a
        sb t0, 5(x1)
        ebreak
    "#;

    #[test]
    fn assembles_addi_sb_and_ebreak() {
        let bytes = assemble(
            r#"
            addi x1, x0, 0x100
            addi t0, zero, -1
            sb t0, 4(x1)
            ebreak
            "#,
        )
        .unwrap();

        assert_eq!(bytes.len(), 16);
        assert_eq!(decode_all(&bytes).len(), 4);
    }

    #[test]
    fn hello_world_fixture_assembles_to_expected_instruction_count() {
        let bytes = assemble(HELLO_SOURCE).unwrap();
        assert_eq!(bytes.len(), 13 * 4);
        assert_eq!(decode_all(&bytes).last(), Some(&Instruction::Ebreak));
    }

    #[test]
    fn assemble_defaults_to_rv32i_profile() {
        let bytes = assemble("addi x31, x0, 1").unwrap();
        assert_eq!(
            decode_all(&bytes),
            vec![Instruction::OpImm {
                op: ImmOp::Addi,
                rd: Reg::X31,
                rs1: Reg::X0,
                imm: 1,
            }]
        );
    }

    #[test]
    fn assemble_with_profile_rejects_rv32e_high_registers() {
        assert_eq!(
            assemble_with_profile("addi x16, x0, 1", IsaProfile::RV32E).unwrap_err(),
            AsmError {
                line_no: 1,
                kind: AsmErrorKind::ProfileViolation,
            }
        );
        assert_eq!(
            assemble_with_profile("sb x1, 0(x31)", IsaProfile::RV32E).unwrap_err(),
            AsmError {
                line_no: 1,
                kind: AsmErrorKind::ProfileViolation,
            }
        );
    }

    #[test]
    fn assemble_with_profile_accepts_rv32i_hello_world() {
        assert_eq!(
            assemble_with_profile(HELLO_SOURCE, IsaProfile::RV32I).unwrap(),
            assemble(HELLO_SOURCE).unwrap()
        );
    }

    #[test]
    fn supports_comments_and_hex_literals() {
        let bytes = assemble("addi a0, zero, 0x2a ; comment\n# full line\n").unwrap();
        assert_eq!(
            decode_all(&bytes),
            vec![Instruction::OpImm {
                op: ImmOp::Addi,
                rd: Reg::X10,
                rs1: Reg::X0,
                imm: 42,
            }]
        );
    }

    #[test]
    fn supports_li_pseudo_instruction_for_small_and_full_width_constants() {
        let bytes = assemble(
            r#"
            li x1, 42
            li x2, 0x60000000
            li x3, 0x40013800
            "#,
        )
        .unwrap();

        assert_eq!(
            decode_all(&bytes),
            vec![
                Instruction::OpImm {
                    op: ImmOp::Addi,
                    rd: Reg::X1,
                    rs1: Reg::X0,
                    imm: 42,
                },
                Instruction::Lui {
                    rd: Reg::X2,
                    imm: 0x6000_0000,
                },
                Instruction::Lui {
                    rd: Reg::X3,
                    imm: 0x4001_4000,
                },
                Instruction::OpImm {
                    op: ImmOp::Addi,
                    rd: Reg::X3,
                    rs1: Reg::X3,
                    imm: -2048,
                },
            ]
        );
    }

    #[test]
    fn supports_word_stores_for_mmio_programs() {
        let bytes = assemble(
            r#"
            li x1, 0x60000000
            li x2, 0x68
            sw x2, 0(x1)
            ebreak
            "#,
        )
        .unwrap();

        assert_eq!(
            decode_all(&bytes),
            vec![
                Instruction::Lui {
                    rd: Reg::X1,
                    imm: 0x6000_0000,
                },
                Instruction::OpImm {
                    op: ImmOp::Addi,
                    rd: Reg::X2,
                    rs1: Reg::X0,
                    imm: 0x68,
                },
                Instruction::Store {
                    width: StoreWidth::Word,
                    rs1: Reg::X1,
                    rs2: Reg::X2,
                    offset: 0,
                },
                Instruction::Ebreak,
            ]
        );
    }

    #[test]
    fn supports_word_loads_for_mmio_programs() {
        let bytes = assemble(
            r#"
            li x1, 0x60004000
            lw x3, 8(x1)
            ebreak
            "#,
        )
        .unwrap();

        assert_eq!(
            decode_all(&bytes),
            vec![
                Instruction::Lui {
                    rd: Reg::X1,
                    imm: 0x6000_4000,
                },
                Instruction::Load {
                    width: LoadWidth::Word,
                    rd: Reg::X3,
                    rs1: Reg::X1,
                    offset: 8,
                },
                Instruction::Ebreak,
            ]
        );
    }

    #[test]
    fn li_pseudo_instruction_is_profile_checked_after_expansion() {
        assert_eq!(
            assemble_with_profile("li x16, 1", IsaProfile::RV32E).unwrap_err(),
            AsmError {
                line_no: 1,
                kind: AsmErrorKind::ProfileViolation,
            }
        );
    }

    #[test]
    fn reports_invalid_source_with_line_number() {
        assert_eq!(
            assemble("addi x1, nope, 1").unwrap_err(),
            AsmError {
                line_no: 1,
                kind: AsmErrorKind::InvalidRegister,
            }
        );
        assert_eq!(
            assemble("\nunknown x1\n").unwrap_err(),
            AsmError {
                line_no: 2,
                kind: AsmErrorKind::UnsupportedMnemonic,
            }
        );
        assert_eq!(
            assemble("sb x1, 0[x2]").unwrap_err(),
            AsmError {
                line_no: 1,
                kind: AsmErrorKind::InvalidMemoryOperand,
            }
        );
    }

    fn decode_all(bytes: &[u8]) -> Vec<Instruction> {
        bytes
            .chunks_exact(4)
            .map(|chunk| {
                let word = u32::from_le_bytes(chunk.try_into().unwrap());
                decode_word(word).unwrap()
            })
            .collect()
    }
}
