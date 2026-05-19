//! Minimal RV32I assembler for the hello-world MVP.

use std::fmt;
use std::str::FromStr;

use sw_rv32i_isa::{ImmOp, Instruction, Reg, StoreWidth, encode_word};

pub fn assemble(source: &str) -> Result<Vec<u8>, AsmError> {
    let mut bytes = Vec::new();
    for (line_index, raw_line) in source.lines().enumerate() {
        let line_no = line_index + 1;
        let line = strip_comment(raw_line).trim();
        if line.is_empty() {
            continue;
        }
        let insn = parse_instruction(line).map_err(|kind| AsmError { line_no, kind })?;
        let word = encode_word(insn).map_err(|_| AsmError {
            line_no,
            kind: AsmErrorKind::InvalidOperands,
        })?;
        bytes.extend_from_slice(&word.to_le_bytes());
    }
    Ok(bytes)
}

fn parse_instruction(line: &str) -> Result<Instruction, AsmErrorKind> {
    let (mnemonic, rest) = split_mnemonic(line)?;
    match mnemonic {
        "addi" => parse_addi(rest),
        "sb" => parse_sb(rest),
        "ebreak" if rest.trim().is_empty() => Ok(Instruction::Ebreak),
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
        imm: parse_literal(operands[2])?,
    })
}

fn parse_sb(rest: &str) -> Result<Instruction, AsmErrorKind> {
    let operands = split_operands(rest);
    if operands.len() != 2 {
        return Err(AsmErrorKind::WrongOperandCount);
    }
    let (offset, base) = parse_offset_base(operands[1])?;
    Ok(Instruction::Store {
        width: StoreWidth::Byte,
        rs1: base,
        rs2: parse_reg(operands[0])?,
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
    let offset = parse_literal(text[..open].trim())?;
    let base = parse_reg(text[open + 1..close].trim())?;
    Ok((offset, base))
}

fn parse_reg(text: &str) -> Result<Reg, AsmErrorKind> {
    Reg::from_str(text).map_err(|_| AsmErrorKind::InvalidRegister)
}

fn parse_literal(text: &str) -> Result<i32, AsmErrorKind> {
    let text = text.trim();
    if text.is_empty() {
        return Err(AsmErrorKind::InvalidLiteral);
    }
    let (negative, digits) = text
        .strip_prefix('-')
        .map_or((false, text), |rest| (true, rest));
    let value = if let Some(hex) = digits.strip_prefix("0x") {
        i32::from_str_radix(hex, 16).map_err(|_| AsmErrorKind::InvalidLiteral)?
    } else {
        digits
            .parse::<i32>()
            .map_err(|_| AsmErrorKind::InvalidLiteral)?
    };
    Ok(if negative { -value } else { value })
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
