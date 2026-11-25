//! A named image a camera draws into, as a source names it: "render:NAME", "render:NAME:WxH", and after that ":float"
//! for an Rgba16Float image (a GPGPU pass's values, unclamped and signed) or ":depth" for a depth one. Every source that
//! names it gets the same image, so a camera's target, a material's map and a three shader's texture meet in it
//! (docs/r3f.feature R3F23a).

use super::*;
use bevy::render::render_resource::TextureFormat;

/// What a render image holds.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RenderKind {
    /// colour as Rgba8UnormSrgb: a camera writes linear values and a material reads them back linear
    Colour,
    /// Rgba16Float: what a camera with Hdr and no tone curve wrote, as it wrote it
    Float,
    /// Depth32Float, sampled as texture_depth_2d (a camera's DepthCopy copies its depth into it)
    Depth,
}

impl RenderImages {
    /// A "render:..." source's name, its size when it gives one and its kind; Err for a source that isn't one.
    pub fn spec(src: &str) -> Result<(&str, Option<(u32, u32)>, RenderKind), String> {
        let rest = src.strip_prefix("render:").ok_or_else(|| format!("{src:?} isn't a render image (\"render:NAME\")"))?;
        let (rest, kind) = match rest.rsplit_once(':') {
            Some((r, "float")) => (r, RenderKind::Float),
            Some((r, "depth")) => (r, RenderKind::Depth),
            _ => (rest, RenderKind::Colour),
        };
        let (name, size) = match rest.rsplit_once(':') {
            Some((n, sz)) if sz.contains('x') => (n, Some(sz)),
            _ => (rest, None),
        };
        if name.is_empty() {
            return Err(format!("{src:?}: a render image needs a name (\"render:mirror\")"));
        }
        let size = match size {
            Some(sz) => {
                let (w, h) = sz.split_once('x').unwrap_or_default();
                let px = |d: &str| d.parse::<u32>().ok().filter(|n| (1..=8192).contains(n));
                Some(px(w).zip(px(h)).ok_or_else(|| format!("{src:?}: a render image's size is WxH in pixels (1 to 8192)"))?)
            }
            None => None,
        };
        Ok((name, size, kind))
    }

    /// A "render:..." source's name (`spec`'s), None for any other source.
    pub fn name(src: &str) -> Option<&str> {
        Self::spec(src).ok().map(|(n, _, _)| n)
    }

    /// The image a "render:..." source names, made the first time one names it (512 x 512 when it gives no size); a
    /// later source giving another size is refused. `depth` makes a depth image (raea-sys-shader's depth_image); None
    /// refuses one, where a depth image can't go (a camera's target, a material's map).
    pub fn image(world: &mut World, src: &str, depth: Option<fn(u32, u32) -> Image>) -> Result<Handle<Image>, String> {
        let (name, size, kind) = Self::spec(src)?;
        let (w, h) = size.unwrap_or((512, 512));
        if let Some(had) = world.get_resource::<RenderImages>().and_then(|r| r.0.get(name)) {
            if size.is_some() && had.1 != (w, h) {
                return Err(format!("{src:?}: the render image {name:?} is already {}x{}", had.1.0, had.1.1));
            }
            return Ok(had.0.clone());
        }
        let img = match kind {
            RenderKind::Depth => depth.ok_or_else(|| format!("{src:?}: a depth image can't be used here"))?(w, h),
            RenderKind::Float => Image::new_target_texture(w, h, TextureFormat::Rgba16Float, None),
            RenderKind::Colour => Image::new_target_texture(w, h, TextureFormat::Rgba8UnormSrgb, None),
        };
        let hd = world.resource_mut::<Assets<Image>>().add(img);
        world.get_resource_or_init::<RenderImages>().0.insert(name.to_string(), (hd.clone(), (w, h)));
        Ok(hd)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_render_source_says_its_name_size_and_kind() {
        assert_eq!(RenderImages::spec("render:mirror").unwrap(), ("mirror", None, RenderKind::Colour));
        assert_eq!(RenderImages::spec("render:a-b:256x128").unwrap(), ("a-b", Some((256, 128)), RenderKind::Colour));
        assert_eq!(RenderImages::spec("render:sim:64x64:float").unwrap(), ("sim", Some((64, 64)), RenderKind::Float));
        assert_eq!(RenderImages::spec("render:z:8x8:depth").unwrap(), ("z", Some((8, 8)), RenderKind::Depth));
        assert_eq!(RenderImages::name("render:z:8x8:depth"), Some("z"));
        assert!(RenderImages::spec("render::8x8").is_err());
        assert!(RenderImages::spec("render:x:0x8").is_err());
        assert!(RenderImages::spec("pic.png").is_err());
    }

    #[test]
    fn every_source_naming_an_image_gets_the_same_one() {
        let mut world = World::new();
        world.init_resource::<Assets<Image>>();
        let a = RenderImages::image(&mut world, "render:sim:64x32:float", None).unwrap();
        let b = RenderImages::image(&mut world, "render:sim", None).unwrap();
        assert_eq!(a, b);
        assert_eq!(world.resource::<Assets<Image>>().get(&a).unwrap().texture_descriptor.format, TextureFormat::Rgba16Float);
        assert!(RenderImages::image(&mut world, "render:sim:16x16", None).is_err());
        assert!(RenderImages::image(&mut world, "render:d:8x8:depth", None).is_err());
    }
}
