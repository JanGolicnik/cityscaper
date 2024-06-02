use jandering_engine::types::Vec3;

pub fn parse_colors(colors: &[(u32, Vec3)]) -> Vec<Vec3> {
    if let Some(last) = colors.last() {
        let n_colors = last.0;
        let mut color_lut = Vec::with_capacity(n_colors as usize);

        let mut current_color_i = 0;
        for i in 0..=n_colors {
            let current_color = &colors[current_color_i];
            let color = if let Some(next) = colors.iter().find(|e| e.0 > current_color.0) {
                if next.0 == i {
                    current_color_i += 1;
                }

                let t = (i - current_color.0) as f32 / (next.0 - current_color.0) as f32;
                Vec3::from(current_color.1) * (1.0 - t) + Vec3::from(next.1) * t
            } else {
                Vec3::from(current_color.1)
            };
            color_lut.push(color);
        }

        color_lut
    } else {
        Vec::new()
    }
}

pub fn parse_colors_linear(colors: &[(u32, Vec3)]) -> Vec<Vec3> {
    colors.iter().map(|(_, color)| *color).collect()
}
