use eframe::egui::{Color32, ColorImage};
use qrcode::{Color, QrCode};

pub type QrError = qrcode::types::QrError;

/// Generates a 2D boolean matrix of QR modules (true = dark, false = light) including a quiet zone.
pub fn generate_qr_matrix(data: &str) -> Result<Vec<Vec<bool>>, QrError> {
    let code = QrCode::new(data.as_bytes())?;
    let colors = code.to_colors();
    let width = code.width();
    let quiet_zone = 2;
    let total_size = width + quiet_zone * 2;

    let mut matrix = vec![vec![false; total_size]; total_size];

    for y in 0..width {
        for x in 0..width {
            let is_dark = colors[y * width + x] == Color::Dark;
            matrix[y + quiet_zone][x + quiet_zone] = is_dark;
        }
    }

    Ok(matrix)
}

/// Renders a terminal-friendly QR code using Unicode full blocks and ANSI colors.
pub fn render_terminal_qr(data: &str) -> Result<String, QrError> {
    let code = QrCode::new(data.as_bytes())?;
    let colors = code.to_colors();
    let width = code.width();
    let quiet_zone = 2;
    let total_size = width + quiet_zone * 2;

    let mut out = String::new();

    // Render using ANSI inverted / double-width blocks for standard aspect ratio
    // Dark modules: "██", Light modules: "  "
    for y in 0..total_size {
        for x in 0..total_size {
            let is_dark = if x >= quiet_zone && x < quiet_zone + width && y >= quiet_zone && y < quiet_zone + width {
                let qx = x - quiet_zone;
                let qy = y - quiet_zone;
                colors[qy * width + qx] == Color::Dark
            } else {
                false
            };

            if is_dark {
                out.push_str("██");
            } else {
                out.push_str("  ");
            }
        }
        out.push('\n');
    }

    Ok(out)
}

/// Renders a high-contrast ColorImage for egui rendering with a quiet zone and configurable scale.
pub fn render_egui_image(data: &str, scale: usize) -> Result<ColorImage, QrError> {
    let code = QrCode::new(data.as_bytes())?;
    let colors = code.to_colors();
    let width = code.width();
    let quiet_zone = 3;
    let total_modules = width + quiet_zone * 2;

    let scale = scale.max(1);
    let image_width = total_modules * scale;
    let image_height = total_modules * scale;

    let mut pixels = Vec::with_capacity(image_width * image_height);

    for my in 0..total_modules {
        for _sy in 0..scale {
            for mx in 0..total_modules {
                let is_dark = if mx >= quiet_zone && mx < quiet_zone + width && my >= quiet_zone && my < quiet_zone + width {
                    let qx = mx - quiet_zone;
                    let qy = my - quiet_zone;
                    colors[qy * width + qx] == Color::Dark
                } else {
                    false
                };

                let color = if is_dark {
                    Color32::from_rgb(18, 18, 20)
                } else {
                    Color32::from_rgb(255, 255, 255)
                };

                for _sx in 0..scale {
                    pixels.push(color);
                }
            }
        }
    }

    Ok(ColorImage {
        size: [image_width, image_height],
        pixels,
    })
}
