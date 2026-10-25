//! One allowlisted registry for source signatures and stable bytecode external IDs.
use crate::layout::Ty;
use crate::vm::ExtFunc;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Library {
    Core,
    Io,
    Math,
}
impl Library {
    pub fn imported(name: &str) -> Option<Self> {
        match name {
            "io" => Some(Self::Io),
            "math" => Some(Self::Math),
            _ => None,
        }
    }
}
#[derive(Debug, Clone, Copy)]
pub enum ValueType {
    Void,
    Int,
    Double,
    Char,
    CharArray,
}
impl ValueType {
    pub fn ty(self) -> Ty {
        match self {
            Self::Void => Ty::Void,
            Self::Int => Ty::Int,
            Self::Double => Ty::Double,
            Self::Char => Ty::Char,
            Self::CharArray => Ty::Array(Box::new(Ty::Char), 0),
        }
    }
}
pub struct Builtin {
    pub name: &'static str,
    pub library: Library,
    pub function: ExtFunc,
    pub wire_id: u64,
    pub result: ValueType,
    pub params: &'static [ValueType],
}
use ExtFunc::*;
use Library::*;
use ValueType::*;
// Existing external IDs are preserved. New IDs must be unique and explicit.
pub const ALL: &[Builtin] = &[
    Builtin {
        name: "put_s",
        library: Io,
        function: PutS,
        wire_id: 0,
        result: Void,
        params: &[CharArray],
    },
    Builtin {
        name: "get_s",
        library: Io,
        function: GetS,
        wire_id: 1,
        result: Void,
        params: &[CharArray],
    },
    Builtin {
        name: "put_i",
        library: Io,
        function: PutI,
        wire_id: 2,
        result: Void,
        params: &[Int],
    },
    Builtin {
        name: "get_i",
        library: Io,
        function: GetI,
        wire_id: 3,
        result: Int,
        params: &[],
    },
    Builtin {
        name: "put_d",
        library: Io,
        function: PutD,
        wire_id: 4,
        result: Void,
        params: &[Double],
    },
    Builtin {
        name: "get_d",
        library: Io,
        function: GetD,
        wire_id: 5,
        result: Double,
        params: &[],
    },
    Builtin {
        name: "put_c",
        library: Io,
        function: PutC,
        wire_id: 6,
        result: Void,
        params: &[Char],
    },
    Builtin {
        name: "get_c",
        library: Io,
        function: GetC,
        wire_id: 7,
        result: Char,
        params: &[],
    },
    Builtin {
        name: "seconds",
        library: Core,
        function: Seconds,
        wire_id: 8,
        result: Double,
        params: &[],
    },
    Builtin {
        name: "sqrt",
        library: Math,
        function: Sqrt,
        wire_id: 9,
        result: Double,
        params: &[Double],
    },
    Builtin {
        name: "pow",
        library: Math,
        function: Pow,
        wire_id: 10,
        result: Double,
        params: &[Double, Double],
    },
    Builtin {
        name: "sin",
        library: Math,
        function: Sin,
        wire_id: 11,
        result: Double,
        params: &[Double],
    },
    Builtin {
        name: "cos",
        library: Math,
        function: Cos,
        wire_id: 12,
        result: Double,
        params: &[Double],
    },
    Builtin {
        name: "tan",
        library: Math,
        function: Tan,
        wire_id: 13,
        result: Double,
        params: &[Double],
    },
    Builtin {
        name: "fabs",
        library: Math,
        function: Fabs,
        wire_id: 14,
        result: Double,
        params: &[Double],
    },
    Builtin {
        name: "floor",
        library: Math,
        function: Floor,
        wire_id: 15,
        result: Double,
        params: &[Double],
    },
    Builtin {
        name: "ceil",
        library: Math,
        function: Ceil,
        wire_id: 16,
        result: Double,
        params: &[Double],
    },
    Builtin {
        name: "round",
        library: Math,
        function: Round,
        wire_id: 17,
        result: Double,
        params: &[Double],
    },
    Builtin {
        name: "exp",
        library: Math,
        function: Exp,
        wire_id: 18,
        result: Double,
        params: &[Double],
    },
    Builtin {
        name: "log",
        library: Math,
        function: Log,
        wire_id: 19,
        result: Double,
        params: &[Double],
    },
    Builtin {
        name: "log10",
        library: Math,
        function: Log10,
        wire_id: 20,
        result: Double,
        params: &[Double],
    },
    Builtin {
        name: "abs",
        library: Math,
        function: Abs,
        wire_id: 21,
        result: Int,
        params: &[Int],
    },
];

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn names_functions_and_wire_ids_are_unique() {
        for (i, a) in ALL.iter().enumerate() {
            for b in &ALL[i + 1..] {
                assert_ne!(a.name, b.name);
                assert_ne!(a.function, b.function);
                assert_ne!(a.wire_id, b.wire_id);
            }
        }
    }
}
