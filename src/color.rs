
#[derive(Debug, PartialEq, Eq, Clone, Copy, Hash)]
pub struct Color { pub r: u8, pub g: u8, pub b: u8 }

impl Color {
    #[inline]
    pub fn new(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b }
    }
    /// Converts RGB to YUV
    // https://en.wikipedia.org/wiki/Y%E2%80%B2UV#Conversion_to/from_RGB
    pub fn to_yuv(&self) -> (u8, u8, u8) {
        let r = self.r as f32; let g = self.g as f32; let b = self.b as f32;
        
        let l = r * 0.299    + g * 0.587   + b * 0.114;
        let u = r * -0.14713 - g * 0.28886 + b * 0.436   + 128.0;
        let v = r * 0.615    - g * 0.51499 - b * 0.10001 + 128.0;

        (l as u8, u as u8, v as u8)    
    }

    pub fn to_cielab(&self) -> (u8, u8, u8) {
        let r = self.r as f32 / 255.0; let g = self.g as f32 / 255.0; let b = self.b as f32 / 255.0;
        
        let r = if r > 0.04045 { ((r + 0.055) / 1.055).powf(2.4) } else { r / 12.92 };
        let g = if g > 0.04045 { ((g + 0.055) / 1.055).powf(2.4) } else { g / 12.92 };
        let b = if b > 0.04045 { ((b + 0.055) / 1.055).powf(2.4) } else { b / 12.92 };

        let x = r * 0.4124564 + g * 0.3575761 + b * 0.1804375;
        let y = r * 0.2126729 + g * 0.7151522 + b * 0.0721750;
        let z = r * 0.0193339 + g * 0.1191920 + b * 0.9503041;

        let x = x / 0.95047;
        let y = y / 1.0;
        let z = z / 1.08883;

        let x = if x > 0.008856 { x.powf(1.0 / 3.0) } else { (903.3 * x + 16.0) / 116.0 };
        let y = if y > 0.008856 { y.powf(1.0 / 3.0) } else { (903.3 * y + 16.0) / 116.0 };
        let z = if z > 0.008856 { z.powf(1.0 / 3.0) } else { (903.3 * z + 16.0) / 116.0 };

        let l = (116.0 * y) - 16.0;
        let a = 500.0 * (x - y);
        let b = 200.0 * (y - z);

        (l as u8, a as u8, b as u8)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use test_case::*;
    
    #[case(255, 255, 255, 255, 128, 128)]
    #[case(0, 0, 0, 0, 128, 128)]
    fn test_color_to_yuv(r: u8, g: u8, b: u8, y: u8, u: u8, v: u8) {
        let c = Color::new(r, g, b);
        assert_eq!(c.to_yuv(), (y, u, v));
    }    
}