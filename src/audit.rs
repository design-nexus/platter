use ratatui::{buffer::Buffer, style::Color};
pub const SIZES: &[(u16, u16)] = &[
    (120, 40),
    (100, 30),
    (80, 24),
    (60, 20),
    (30, 10),
    (20, 8),
    (1, 1),
];
pub fn text(buffer: &Buffer) -> String {
    buffer
        .content
        .chunks(buffer.area.width.max(1) as usize)
        .map(|row| row.iter().map(|c| c.symbol()).collect::<String>())
        .collect::<Vec<_>>()
        .join("\n")
}
fn color(color: Color, foreground: bool) -> String {
    match color {
        Color::Rgb(r, g, b) => format!("#{r:02x}{g:02x}{b:02x}"),
        _ => {
            if foreground {
                "#ececf4".into()
            } else {
                "#171820".into()
            }
        }
    }
}
fn escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}
pub fn capture(app: &str, state: &str, buffer: &Buffer) {
    let Ok(dir) = std::env::var("AURORA_RENDER_DIR") else {
        return;
    };
    let (w, h) = (buffer.area.width, buffer.area.height);
    let path = std::path::Path::new(&dir);
    std::fs::create_dir_all(path).unwrap();
    std::fs::write(
        path.join(format!("{app}-{state}-{w}x{h}.txt")),
        text(buffer),
    )
    .unwrap();
    let mut svg = format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="{}" height="{}"><rect width="100%" height="100%" fill="#171820"/><g font-family="DejaVu Sans Mono" font-size="14">"##,
        w as usize * 9,
        h as usize * 19
    );
    for y in 0..h {
        for x in 0..w {
            let c = &buffer[(x, y)];
            let (px, py) = (x as usize * 9, y as usize * 19);
            svg.push_str(&format!(
                r#"<rect x="{px}" y="{py}" width="9" height="19" fill="{}"/>"#,
                color(c.bg, false)
            ));
        }
    }
    for y in 0..h {
        for x in 0..w {
            let c = &buffer[(x, y)];
            if c.symbol().trim().is_empty() {
                continue;
            }
            svg.push_str(&format!(
                r#"<text x="{}" y="{}" fill="{}">{}</text>"#,
                x as usize * 9,
                y as usize * 19 + 14,
                color(c.fg, true),
                escape(c.symbol())
            ));
        }
    }
    svg.push_str("</g></svg>");
    std::fs::write(path.join(format!("{app}-{state}-{w}x{h}.svg")), svg).unwrap();
}
