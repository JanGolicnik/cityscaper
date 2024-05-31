use std::ops::{Add, Rem, Sub};

use image::Rgb32FImage;
use jandering_engine::types::Vec2;

pub struct Image {
    image: Rgb32FImage,
    width: u32,
    height: u32,
    scale: f32,
}

impl Image {
    pub fn new(image: Rgb32FImage, scale: f32) -> Self {
        let width = image.width();
        let height = image.height();
        Self {
            image,
            width,
            height,
            scale,
        }
    }

    pub fn sample(&self, u: f32, v: f32) -> f32 {
        let u = wrap(u * self.scale, 0.0, 1.0) * self.width as f32;
        let v = wrap(v * self.scale, 0.0, 1.0) * self.height as f32;
        let x = u as i32;
        let y = v as i32;

        let mut vals = Vec::with_capacity(9);

        for i in -1..=1 {
            let y = wrap(y + i, 0, self.height as i32 - 1) as u32;
            for j in -1..=1 {
                let x = wrap(x + j, 0, self.width as i32 - 1) as u32;
                let val = self.image.get_pixel(x, y)[0];
                let dist = (Vec2::new(j as f32, i as f32) * 0.5).distance(Vec2::new(u, v).fract());

                vals.push(((1.0 - dist.min(1.0)) * val, val))
            }
        }

        let sum = vals.iter().fold(0.0, |acc, (e, _)| acc + e);
        vals.into_iter().map(|(e, val)| (e / sum) * val).sum()
    }
}

fn wrap<T>(val: T, min: T, max: T) -> T
where
    T: Copy + PartialEq + PartialOrd + Sub<Output = T> + Rem<Output = T> + Add<Output = T>,
{
    if val < min {
        max + ((val - min) % (max - min))
    } else if val > max {
        min + ((val - max) % (max - min))
    } else {
        val
    }
}
