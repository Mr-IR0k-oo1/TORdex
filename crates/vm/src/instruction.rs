use std::fmt;

use serde::{Deserialize, Serialize};

// ─── Opcodes ─────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[repr(u8)]
pub enum Opcode {
    // Data (0x00–0x0F)
    Nop = 0x00,
    LoadImm = 0x01,
    LoadStr = 0x02,
    Mov = 0x03,
    LoadEntity = 0x04,
    LoadGraph = 0x05,
    Traverse = 0x06,
    Compare = 0x07,

    // Knowledge (0x10–0x1F)
    Fact = 0x10,
    Query = 0x11,
    Relate = 0x12,
    Infer = 0x13,
    Similar = 0x14,
    Classify = 0x15,
    Reason = 0x16,
    Correlate = 0x17,
    Timeline = 0x18,

    // Stack (0x20–0x2F)
    Push = 0x20,
    Pop = 0x21,
    Dup = 0x22,
    Swap = 0x23,

    // Control (0x30–0x3F)
    Jmp = 0x30,
    Jz = 0x31,
    Jnz = 0x32,
    Call = 0x33,
    Ret = 0x34,
    Halt = 0x35,

    // Search (0x40–0x4F)
    Search = 0x40,
    Index = 0x41,
    Rank = 0x42,

    // Memory (0x50–0x5F)
    Recall = 0x50,
    Consolidate = 0x51,
    Forget = 0x52,
    Store = 0x53,
    Snapshot = 0x54,

    // Kernel (0x60–0x6F)
    SysCall = 0x60,
    Emit = 0x61,

}

impl Opcode {
    pub fn from_u8(byte: u8) -> Option<Self> {
        match byte {
            0x00 => Some(Self::Nop),
            0x01 => Some(Self::LoadImm),
            0x02 => Some(Self::LoadStr),
            0x03 => Some(Self::Mov),
            0x04 => Some(Self::LoadEntity),
            0x05 => Some(Self::LoadGraph),
            0x06 => Some(Self::Traverse),
            0x07 => Some(Self::Compare),
            0x10 => Some(Self::Fact),
            0x11 => Some(Self::Query),
            0x12 => Some(Self::Relate),
            0x13 => Some(Self::Infer),
            0x14 => Some(Self::Similar),
            0x15 => Some(Self::Classify),
            0x16 => Some(Self::Reason),
            0x17 => Some(Self::Correlate),
            0x18 => Some(Self::Timeline),
            0x20 => Some(Self::Push),
            0x21 => Some(Self::Pop),
            0x22 => Some(Self::Dup),
            0x23 => Some(Self::Swap),
            0x30 => Some(Self::Jmp),
            0x31 => Some(Self::Jz),
            0x32 => Some(Self::Jnz),
            0x33 => Some(Self::Call),
            0x34 => Some(Self::Ret),
            0x35 => Some(Self::Halt),
            0x40 => Some(Self::Search),
            0x41 => Some(Self::Index),
            0x42 => Some(Self::Rank),
            0x50 => Some(Self::Recall),
            0x51 => Some(Self::Consolidate),
            0x52 => Some(Self::Forget),
            0x53 => Some(Self::Store),
            0x54 => Some(Self::Snapshot),
            0x60 => Some(Self::SysCall),
            0x61 => Some(Self::Emit),
            _ => None,
        }
    }
}

impl fmt::Display for Opcode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}", self)
    }
}

// ─── Instructions ────────────────────────────────────────────────────────────

/// An 8-byte VM instruction.
///
/// Layout: `[opcode: u8, reg_a: u8, reg_b: u8, extra: u8, imm_or_offset: i32]`
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Instruction {
    pub opcode: Opcode,
    pub reg_a: u8,
    pub reg_b: u8,
    pub extra: u8,
    pub imm: i32,
}

impl Instruction {
    pub fn new(opcode: Opcode) -> Self {
        Self {
            opcode,
            reg_a: 0,
            reg_b: 0,
            extra: 0,
            imm: 0,
        }
    }

    pub fn with_reg_a(mut self, reg: u8) -> Self {
        self.reg_a = reg;
        self
    }

    pub fn with_reg_b(mut self, reg: u8) -> Self {
        self.reg_b = reg;
        self
    }

    pub fn with_extra(mut self, val: u8) -> Self {
        self.extra = val;
        self
    }

    pub fn with_imm(mut self, val: i32) -> Self {
        self.imm = val;
        self
    }

    /// Encode this instruction into an 8-byte array.
    pub fn encode(&self) -> [u8; 8] {
        let mut buf = [0u8; 8];
        buf[0] = self.opcode as u8;
        buf[1] = self.reg_a;
        buf[2] = self.reg_b;
        buf[3] = self.extra;
        buf[4..8].copy_from_slice(&self.imm.to_le_bytes());
        buf
    }

    /// Decode an 8-byte array into an instruction.
    pub fn decode(bytes: &[u8; 8]) -> Option<Self> {
        let opcode = Opcode::from_u8(bytes[0])?;
        let imm = i32::from_le_bytes(bytes[4..8].try_into().ok()?);
        Some(Self {
            opcode,
            reg_a: bytes[1],
            reg_b: bytes[2],
            extra: bytes[3],
            imm,
        })
    }
}

impl fmt::Display for Instruction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.opcode)?;
        match self.opcode {
            Opcode::Nop | Opcode::Halt | Opcode::Ret => {}
            Opcode::LoadImm | Opcode::LoadStr | Opcode::Push | Opcode::Pop | Opcode::Dup => {
                write!(f, " r{}", self.reg_a)?;
                if self.imm != 0 {
                    write!(f, ", {}", self.imm)?;
                }
            }
            Opcode::Mov | Opcode::Swap => {
                write!(f, " r{}, r{}", self.reg_a, self.reg_b)?;
            }
            Opcode::Jmp | Opcode::Jz | Opcode::Jnz => {
                write!(f, " {}", self.imm)?;
            }
            Opcode::Call => {
                write!(f, " {}", self.imm)?;
            }
            Opcode::Fact | Opcode::Relate | Opcode::Infer | Opcode::Similar => {
                write!(f, " r{}, r{}, r{}", self.reg_a, self.reg_b, self.extra)?;
            }
            Opcode::Query | Opcode::Search | Opcode::Correlate | Opcode::Compare => {
                write!(f, " r{}, r{}", self.reg_a, self.reg_b)?;
            }
            Opcode::Classify | Opcode::Reason | Opcode::Timeline | Opcode::Traverse => {
                write!(f, " r{}, r{}", self.reg_a, self.reg_b)?;
            }
            Opcode::LoadEntity | Opcode::LoadGraph | Opcode::Store | Opcode::Snapshot => {
                write!(f, " r{}", self.reg_a)?;
            }
            Opcode::SysCall => {
                write!(f, " {}", self.imm)?;
            }
            _ => {
                write!(f, " r{}, r{}, {}", self.reg_a, self.reg_b, self.imm)?;
            }
        }
        Ok(())
    }
}

// ─── Instruction Builder Helpers ─────────────────────────────────────────────

pub fn nop() -> Instruction {
    Instruction::new(Opcode::Nop)
}

pub fn load_imm(reg: u8, value: i32) -> Instruction {
    Instruction::new(Opcode::LoadImm).with_reg_a(reg).with_imm(value)
}

pub fn load_str(reg: u8, string_idx: u16) -> Instruction {
    Instruction::new(Opcode::LoadStr)
        .with_reg_a(reg)
        .with_imm(string_idx as i32)
}

pub fn mov(dst: u8, src: u8) -> Instruction {
    Instruction::new(Opcode::Mov).with_reg_a(dst).with_reg_b(src)
}

pub fn fact(subj: u8, pred: u8, obj: u8) -> Instruction {
    Instruction::new(Opcode::Fact)
        .with_reg_a(subj)
        .with_reg_b(pred)
        .with_extra(obj)
}

pub fn query(dst: u8, query_reg: u8) -> Instruction {
    Instruction::new(Opcode::Query).with_reg_a(dst).with_reg_b(query_reg)
}

pub fn relate(src: u8, kind: u8, tgt: u8) -> Instruction {
    Instruction::new(Opcode::Relate)
        .with_reg_a(src)
        .with_reg_b(kind)
        .with_extra(tgt)
}

pub fn infer(dst: u8, subj: u8, pred: u8) -> Instruction {
    Instruction::new(Opcode::Infer)
        .with_reg_a(dst)
        .with_reg_b(subj)
        .with_extra(pred)
}

pub fn push(reg: u8) -> Instruction {
    Instruction::new(Opcode::Push).with_reg_a(reg)
}

pub fn pop(reg: u8) -> Instruction {
    Instruction::new(Opcode::Pop).with_reg_a(reg)
}

pub fn dup() -> Instruction {
    Instruction::new(Opcode::Dup)
}

pub fn swap() -> Instruction {
    Instruction::new(Opcode::Swap)
}

pub fn jmp(offset: i32) -> Instruction {
    Instruction::new(Opcode::Jmp).with_imm(offset)
}

pub fn jz(reg: u8, offset: i32) -> Instruction {
    Instruction::new(Opcode::Jz).with_reg_a(reg).with_imm(offset)
}

pub fn jnz(reg: u8, offset: i32) -> Instruction {
    Instruction::new(Opcode::Jnz).with_reg_a(reg).with_imm(offset)
}

pub fn call(offset: i32) -> Instruction {
    Instruction::new(Opcode::Call).with_imm(offset)
}

pub fn ret() -> Instruction {
    Instruction::new(Opcode::Ret)
}

pub fn halt() -> Instruction {
    Instruction::new(Opcode::Halt)
}

pub fn search(dst: u8, idx: u8) -> Instruction {
    Instruction::new(Opcode::Search).with_reg_a(dst).with_reg_b(idx)
}

pub fn index_(reg: u8, idx: u8) -> Instruction {
    Instruction::new(Opcode::Index).with_reg_a(reg).with_reg_b(idx)
}

pub fn rank(dst: u8, results: u8) -> Instruction {
    Instruction::new(Opcode::Rank).with_reg_a(dst).with_reg_b(results)
}

pub fn recall(dst: u8, id_reg: u8) -> Instruction {
    Instruction::new(Opcode::Recall).with_reg_a(dst).with_reg_b(id_reg)
}

pub fn consolidate() -> Instruction {
    Instruction::new(Opcode::Consolidate)
}

pub fn forget(reg: u8) -> Instruction {
    Instruction::new(Opcode::Forget).with_reg_a(reg)
}

pub fn syscall(number: i32) -> Instruction {
    Instruction::new(Opcode::SysCall).with_imm(number)
}

pub fn emit(ev_reg: u8) -> Instruction {
    Instruction::new(Opcode::Emit).with_reg_a(ev_reg)
}

// ─── New Intelligence ISA Helpers ─────────────────────────────────────────────

pub fn load_entity(dst: u8, kind_reg: u8) -> Instruction {
    Instruction::new(Opcode::LoadEntity).with_reg_a(dst).with_reg_b(kind_reg)
}

pub fn load_graph(dst: u8, entity_reg: u8) -> Instruction {
    Instruction::new(Opcode::LoadGraph).with_reg_a(dst).with_reg_b(entity_reg)
}

pub fn traverse(dst: u8, entity_reg: u8) -> Instruction {
    Instruction::new(Opcode::Traverse).with_reg_a(dst).with_reg_b(entity_reg)
}

pub fn similar(dst: u8, value_reg: u8, threshold_reg: u8) -> Instruction {
    Instruction::new(Opcode::Similar)
        .with_reg_a(dst)
        .with_reg_b(value_reg)
        .with_extra(threshold_reg)
}

pub fn classify(dst: u8, content_reg: u8) -> Instruction {
    Instruction::new(Opcode::Classify).with_reg_a(dst).with_reg_b(content_reg)
}

pub fn reason(dst: u8, fact_reg: u8) -> Instruction {
    Instruction::new(Opcode::Reason).with_reg_a(dst).with_reg_b(fact_reg)
}

pub fn correlate(dst: u8, ent_a: u8) -> Instruction {
    Instruction::new(Opcode::Correlate).with_reg_a(dst).with_reg_b(ent_a)
}

pub fn timeline(dst: u8, query_reg: u8) -> Instruction {
    Instruction::new(Opcode::Timeline).with_reg_a(dst).with_reg_b(query_reg)
}

pub fn store_val(reg: u8) -> Instruction {
    Instruction::new(Opcode::Store).with_reg_a(reg)
}

pub fn snapshot(dst: u8, kind: u8) -> Instruction {
    Instruction::new(Opcode::Snapshot).with_reg_a(dst).with_reg_b(kind)
}

pub fn compare_(dst: u8, val_a: u8) -> Instruction {
    Instruction::new(Opcode::Compare).with_reg_a(dst).with_reg_b(val_a)
}
