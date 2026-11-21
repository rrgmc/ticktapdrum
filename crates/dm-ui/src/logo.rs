//! The app's logo, the desktop icon drawn by the dm-icons tool (see `tools/icons`).

static PNG: &[u8] = include_bytes!("../assets/logo-256.png");

/// The logo's texture, decoded and uploaded once per context. `None` if the image cannot be
/// decoded, which the tests rule out.
pub(crate) fn texture(ctx: &egui::Context) -> Option<egui::TextureHandle> {
    let id = egui::Id::unique("dm-ui-logo");
    if let Some(texture) = ctx.data(|d| d.get_temp::<egui::TextureHandle>(id)) {
        return Some(texture);
    }
    let image = decode()?;
    let texture = ctx.load_texture("logo", image, egui::TextureOptions::LINEAR);
    ctx.data_mut(|d| d.insert_temp(id, texture.clone()));
    Some(texture)
}

fn decode() -> Option<egui::ColorImage> {
    let mut decoder = png::Decoder::new(std::io::Cursor::new(PNG));
    decoder.set_transformations(png::Transformations::normalize_to_color8());
    let mut reader = decoder.read_info().ok()?;
    let mut buf = vec![0; reader.output_buffer_size()?];
    let info = reader.next_frame(&mut buf).ok()?;
    if info.color_type != png::ColorType::Rgba {
        return None;
    }
    let size = [info.width as usize, info.height as usize];
    Some(egui::ColorImage::from_rgba_unmultiplied(
        size,
        &buf[..info.buffer_size()],
    ))
}

#[cfg(test)]
mod tests {
    #[test]
    fn the_logo_decodes_as_a_square() {
        let image = super::decode().expect("the logo decodes");
        assert_eq!(image.size, [256, 256]);
    }
}
