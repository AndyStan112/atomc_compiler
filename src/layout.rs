//! Fixed guest ABI; independent of the host architecture.
use std::collections::BTreeMap;

pub const MAX_OBJECT: usize = 32 * 1024;
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Ty {
    Int,
    Double,
    Char,
    Void,
    Struct(String),
    Array(Box<Ty>, usize),
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Layout {
    pub size: usize,
    pub align: usize,
}
#[derive(Debug, Clone)]
pub struct Field {
    pub name: String,
    pub ty: Ty,
    pub offset: usize,
}
#[derive(Debug, Clone)]
pub struct StructLayout {
    pub layout: Layout,
    pub fields: Vec<Field>,
}
pub type Structs = BTreeMap<String, StructLayout>;

pub fn align_up(value: usize, align: usize) -> Result<usize, String> {
    if !align.is_power_of_two() {
        return Err("invalid alignment".into());
    }
    value
        .checked_add(align - 1)
        .map(|n| n & !(align - 1))
        .ok_or("layout overflow".into())
}
pub fn layout(ty: &Ty, structs: &Structs) -> Result<Layout, String> {
    let result = match ty {
        Ty::Char => Layout { size: 1, align: 1 },
        Ty::Int => Layout { size: 4, align: 4 },
        Ty::Double => Layout { size: 8, align: 8 },
        Ty::Void => return Err("void has no storage".into()),
        Ty::Struct(name) => structs
            .get(name)
            .ok_or(format!("incomplete or recursive struct {name}"))?
            .layout
            .clone(),
        Ty::Array(element, count) => {
            if *count == 0 {
                return Err("array requires a positive dimension".into());
            }
            let e = layout(element, structs)?;
            Layout {
                size: e.size.checked_mul(*count).ok_or("array layout overflow")?,
                align: e.align,
            }
        }
    };
    if result.size > MAX_OBJECT {
        return Err("object exceeds guest memory limit".into());
    }
    Ok(result)
}
pub fn structure(fields: Vec<(String, Ty)>, structs: &Structs) -> Result<StructLayout, String> {
    if fields.is_empty() {
        return Err("empty structs are unsupported".into());
    }
    let mut size = 0usize;
    let mut align = 1;
    let mut result: Vec<Field> = Vec::new();
    for (name, ty) in fields {
        if result.iter().any(|f| f.name == name) {
            return Err("duplicate field".into());
        }
        let l = layout(&ty, structs)?;
        align = align.max(l.align);
        let offset = align_up(size, l.align)?;
        size = offset.checked_add(l.size).ok_or("struct layout overflow")?;
        result.push(Field { name, ty, offset });
    }
    size = align_up(size, align)?;
    if size > MAX_OBJECT {
        return Err("struct exceeds guest memory limit".into());
    }
    Ok(StructLayout {
        layout: Layout { size, align },
        fields: result,
    })
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn padding_nested_arrays_and_tail_padding() {
        let mut s = Structs::new();
        let p = structure(
            vec![
                ("c".into(), Ty::Char),
                ("d".into(), Ty::Double),
                ("i".into(), Ty::Int),
            ],
            &s,
        )
        .unwrap();
        assert_eq!(
            p.fields.iter().map(|f| f.offset).collect::<Vec<_>>(),
            [0, 8, 16]
        );
        assert_eq!(p.layout, Layout { size: 24, align: 8 });
        s.insert("P".into(), p);
        assert_eq!(
            layout(&Ty::Array(Box::new(Ty::Struct("P".into())), 3), &s)
                .unwrap()
                .size,
            72
        );
    }
    #[test]
    fn rejects_incomplete_and_oversized_objects() {
        let s = Structs::new();
        assert!(layout(&Ty::Struct("Self".into()), &s).is_err());
        assert!(layout(&Ty::Array(Box::new(Ty::Double), usize::MAX), &s).is_err());
        assert!(layout(&Ty::Array(Box::new(Ty::Int), 0), &s).is_err());
        assert!(align_up(usize::MAX, 8).is_err());
    }
}
