use std::fmt::Write as _;

use crate::color::Color;


#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub struct Pixel {
    pub x: usize,
    pub y: usize,
    pub color: Color,
}

impl Pixel {    
    pub fn to_pixelflut_string(&self, offset_x: usize, offset_y: usize) -> String {
        
        let x = self.x + offset_x;
        let y = self.y + offset_y;
        // PX(2) + 1 + X(4) + 1 + Y(4) + 1 + C(6) + \n(1) = 19
        let mut result = String::with_capacity(24);
        write!(result, "PX ").unwrap();
        write!(result, "{} ", x).unwrap();
        write!(result, "{} ", y).unwrap();
        write!(result, "{:02x}{:02x}{:02x}\n", self.color.r, self.color.g, self.color.b).unwrap();
        
        // let result = format!("PX {} {} {:02x}{:02x}{:02x}\n", self.x + offset_x, self.y + offset_y, self.color.r, self.color.g, self.color.b);
        result
    }
}

