use std::fmt;
use std::num::NonZeroUsize;

use libafl::inputs::BytesInput;
use libafl_bolts::rands::Rand;

// ── AST types ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum Op {
    Add,
    Sub,
    Mul,
    Div,
    Mod,
    Power,
    BitOr,
    BitXor,
    BitAnd,
    Shl,
    Shr,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum NumLit {
    Decimal(f64),
    Hex(i64),
    HexUpper(i64),
    Binary(i64),
    BinaryUpper(i64),
    LeadingDot(u32),
    BareZero,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum Expr {
    Num(NumLit),
    UnaryNeg(Box<Expr>),
    BinOp {
        left: Box<Expr>,
        op: Op,
        right: Box<Expr>,
    },
    Paren(Box<Expr>),
}

pub const ALL_OPS: [Op; 11] = [
    Op::Add,
    Op::Sub,
    Op::Mul,
    Op::Div,
    Op::Mod,
    Op::Power,
    Op::BitOr,
    Op::BitXor,
    Op::BitAnd,
    Op::Shl,
    Op::Shr,
];

// ── Display: AST → calculator input string ───────────────────────────────────

impl fmt::Display for Op {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Op::Add => "+",
            Op::Sub => "-",
            Op::Mul => "*",
            Op::Div => "/",
            Op::Mod => "%",
            Op::Power => "**",
            Op::BitOr => "|",
            Op::BitXor => "^",
            Op::BitAnd => "&",
            Op::Shl => "<<",
            Op::Shr => ">>",
        })
    }
}

impl fmt::Display for NumLit {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            NumLit::Decimal(v) => {
                if v.is_finite() && *v == (*v as i64) as f64 {
                    write!(f, "{}", *v as i64)
                } else if v.is_finite() {
                    write!(f, "{}", v)
                } else {
                    write!(f, "1")
                }
            }
            NumLit::Hex(v) => write!(f, "0x{:X}", v.unsigned_abs()),
            NumLit::HexUpper(v) => write!(f, "0X{:X}", v.unsigned_abs()),
            NumLit::Binary(v) => write!(f, "0b{:b}", v.unsigned_abs()),
            NumLit::BinaryUpper(v) => write!(f, "0B{:b}", v.unsigned_abs()),
            NumLit::LeadingDot(frac) => write!(f, ".{}", frac),
            NumLit::BareZero => write!(f, "0"),
        }
    }
}

impl fmt::Display for Expr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Expr::Num(n) => write!(f, "{}", n),
            Expr::UnaryNeg(e) => write!(f, "-{}", e),
            Expr::BinOp { left, op, right } => write!(f, "{}{}{}", left, op, right),
            Expr::Paren(e) => write!(f, "({})", e),
        }
    }
}

// ── Generator ────────────────────────────────────────────────────────────────

fn rand_below(rng: &mut impl Rand, upper: usize) -> usize {
    if upper <= 1 {
        return 0;
    }
    rng.below(NonZeroUsize::new(upper).unwrap())
}

pub struct ExprGenerator {
    max_depth: usize,
}

impl ExprGenerator {
    pub fn new(max_depth: usize) -> Self {
        Self { max_depth }
    }

    pub fn random_expr(&self, rng: &mut impl Rand, depth: usize) -> Expr {
        if depth >= self.max_depth {
            return Expr::Num(self.random_numlit(rng));
        }

        match rand_below(rng, 100) {
            0..35 => Expr::Num(self.random_numlit(rng)),
            35..70 => {
                let left = Box::new(self.random_expr(rng, depth + 1));
                let op = ALL_OPS[rand_below(rng, ALL_OPS.len())];
                let right = Box::new(self.random_expr(rng, depth + 1));
                Expr::BinOp { left, op, right }
            }
            70..82 => Expr::Paren(Box::new(self.random_expr(rng, depth + 1))),
            82..95 => Expr::UnaryNeg(Box::new(self.random_expr(rng, depth + 1))),
            _ => self.dangerous_expr(rng),
        }
    }

    fn random_numlit(&self, rng: &mut impl Rand) -> NumLit {
        match rand_below(rng, 100) {
            0..20 => NumLit::Decimal(rand_below(rng, 1000) as f64),
            20..30 => {
                NumLit::Decimal(rand_below(rng, 100) as f64 + rand_below(rng, 100) as f64 / 100.0)
            }
            30..40 => NumLit::Hex(rand_below(rng, 0xFFFF) as i64),
            40..50 => NumLit::HexUpper(rand_below(rng, 0xFFFF) as i64),
            50..60 => NumLit::Binary(rand_below(rng, 256) as i64),
            60..70 => NumLit::BinaryUpper(rand_below(rng, 256) as i64),
            70..80 => NumLit::LeadingDot(rand_below(rng, 1000) as u32),
            80..90 => NumLit::BareZero,
            _ => self.edge_case_number(rng),
        }
    }

    fn edge_case_number(&self, rng: &mut impl Rand) -> NumLit {
        NumLit::Decimal(match rand_below(rng, 18) {
            0 => 0.0,
            1 => i32::MAX as f64,
            2 => i32::MIN as f64,
            3 => (i32::MAX as f64) + 1.0,
            4 => (i32::MIN as f64) - 1.0,
            5 => i64::MAX as f64,
            6 => i64::MIN as f64,
            7 => 64.0,
            8 => 65.0,
            9 => 63.0,
            10 => 128.0,
            11 => 1.0,
            12 => 2.0,
            13 => 1e18,
            14 => 1e19,
            15 => 1e20,
            16 => 0.1,
            _ => f64::EPSILON,
        })
    }

    fn dangerous_expr(&self, rng: &mut impl Rand) -> Expr {
        let choice = rand_below(rng, 26);

        macro_rules! n {
            () => {
                Expr::Num(self.random_numlit(rng))
            };
        }
        macro_rules! v {
            ($x:expr) => {
                Expr::Num(NumLit::Decimal($x))
            };
        }
        macro_rules! neg {
            ($e:expr) => {
                Expr::UnaryNeg(Box::new($e))
            };
        }
        macro_rules! bin {
            ($l:expr, $op:expr, $r:expr) => {
                Expr::BinOp {
                    left: Box::new($l),
                    op: $op,
                    right: Box::new($r),
                }
            };
        }

        match choice {
            0 => bin!(n!(), Op::Mod, v!(0.0)),
            1 => bin!(n!(), Op::Div, v!(0.0)),
            2 => bin!(n!(), Op::Shl, v!(64.0)),
            3 => bin!(n!(), Op::Shl, v!(128.0)),
            4 => bin!(n!(), Op::Shr, v!(64.0)),
            5 => bin!(neg!(n!()), Op::Shl, n!()),
            6 => bin!(neg!(n!()), Op::Shr, n!()),
            7 => bin!(v!(9999999999999999999.0), Op::BitOr, n!()),
            8 => bin!(v!(9999999999999999999.0), Op::BitAnd, n!()),
            9 => bin!(v!(9999999999999999999.0), Op::BitXor, n!()),
            10 => bin!(v!(9999999999999999999.0), Op::Shl, n!()),
            11 => bin!(neg!(n!()), Op::BitAnd, neg!(n!())),
            12 => bin!(neg!(n!()), Op::BitOr, neg!(n!())),
            13 => bin!(n!(), Op::Power, v!(64.0)),
            14 => bin!(neg!(n!()), Op::Power, v!(2.0)),
            15 => {
                let depth = rand_below(rng, 20) + 5;
                let mut expr = n!();
                for _ in 0..depth {
                    expr = Expr::Paren(Box::new(expr));
                }
                expr
            }
            16 => bin!(bin!(n!(), Op::Add, n!()), Op::Shl, n!()),
            17 => bin!(bin!(n!(), Op::Mul, n!()), Op::Power, n!()),
            18 => bin!(v!(2147483648.0), Op::Mod, n!()),
            19 => bin!(v!(i64::MAX as f64), Op::Mod, n!()),
            // 0**0
            20 => bin!(v!(0.0), Op::Power, v!(0.0)),
            // negative exponent
            21 => bin!(n!(), Op::Power, neg!(n!())),
            // double negation
            22 => neg!(neg!(n!())),
            // negative mod both sides
            23 => bin!(neg!(n!()), Op::Mod, neg!(n!())),
            // negative bitwise xor
            24 => bin!(neg!(n!()), Op::BitXor, neg!(n!())),
            // bare zero in expression
            _ => bin!(Expr::Num(NumLit::BareZero), Op::Add, n!()),
        }
    }
}

impl<S> libafl::generators::Generator<BytesInput, S> for ExprGenerator
where
    S: libafl::state::HasRand,
{
    fn generate(&mut self, state: &mut S) -> Result<BytesInput, libafl::Error> {
        let expr = self.random_expr(state.rand_mut(), 0);
        Ok(BytesInput::new(expr.to_string().into_bytes()))
    }
}
