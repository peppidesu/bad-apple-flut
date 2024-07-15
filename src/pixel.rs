use crate::Color;

#[derive(Debug, PartialEq, Eq, Clone, Copy, Hash)]
pub struct Pixel {
    pub x: usize,
    pub y: usize,
    pub color: Color,
}



