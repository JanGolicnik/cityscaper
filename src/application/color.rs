use colors_transform::Color;
use jandering_engine::types::Vec3;
use serde::Deserialize;

#[derive(Deserialize)]
pub enum ColorValue {
    RGB{
        value: [f32; 3]
    },
    HSL{
        value: [f32; 3]
    }
}

impl ColorValue {
    pub fn get_rgb(&self) -> Vec3{
        match self{
            ColorValue::RGB { value } => Vec3::from(*value),
            ColorValue::HSL { value } => {
                let rgb = colors_transform::Hsl::from(value[0], value[1] * 100.0, value[2] * 100.0).to_rgb();
                Vec3::new(rgb.get_red() / 255.0, rgb.get_green() / 255.0, rgb.get_blue() / 255.0)
            },
        }
    }
}

#[derive(Deserialize)]
pub struct LutColorStop {
    pub color: ColorValue,
    pub age: u32
}

pub struct ColorLut {
    pub colors: Vec<LutColorStop>,
}

impl ColorLut{
    pub fn new(text: &str) -> Self{
        let colors = serde_json::from_str::<Vec<LutColorStop>>(text)
        .unwrap();
        Self {
            colors
        }
    }

    pub fn to_rgb(&self) -> Vec<u8>{
        let mut out = Vec::new();

        let mut previous_color_stop: Option<&LutColorStop> = None;

        for (i, color_stop )in self.colors.iter().enumerate() {
            let start_age = if let Some(previous_color_stop) = previous_color_stop {
                previous_color_stop.age
            }else{
                0
            };

            let this_rgb = color_stop.color.get_rgb();
            let previous_rgb = self.colors[i.saturating_sub(1)].color.get_rgb();

            let age_diff = color_stop.age.saturating_sub(start_age);

            for i in 0..=age_diff {
                let t = i as f32 / age_diff.max(1) as f32;

                let interpolated_color = this_rgb * t + previous_rgb * (1.0 - t);
                out.push(interpolated_color);
            }

            previous_color_stop = Some(color_stop)
        }

        out.into_iter()
        .flat_map(|e| {
            [
                (e.x * 255.0) as u8,
                (e.y * 255.0) as u8,
                (e.z * 255.0) as u8,
                255,
            ]
        })
        .collect()
    }

    pub fn to_rgb_linear(&self) -> Vec<u8>{
        self.colors.iter().map(|e| e.color.get_rgb())
        .flat_map(|e| {
            [
                (e.x * 255.0) as u8,
                (e.y * 255.0) as u8,
                (e.z * 255.0) as u8,
                255,
            ]
        })
        .collect()
    }
}