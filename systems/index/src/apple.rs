//! The fast pass on a Mac (docs/index.feature IX1): a picture's small copy from ImageIO (the embedded preview when it
//! is big enough, else the file decoded at a reduced size), squashed into a 256 x 256 BGRA pixel buffer as SigLIP's
//! processor resizes, a 256 px JPEG thumbnail written beside it, and SigLIP 2 B/16-256 on Core ML's CPU and Neural
//! Engine, a batch at a time. Never Core ML's GPU: on an M5 Pro it computes SigLIP 2 wrong (coreml-gpu-siglip).

use objc2::rc::Retained;
use objc2::runtime::{AnyObject, ProtocolObject};
use objc2::AnyThread;
use objc2_core_foundation::{CFBoolean, CFDictionary, CFNumber, CFRetained, CFString, CFType, CFURL};
use objc2_core_graphics::{CGBitmapContextCreate, CGColorSpace, CGContext, CGImage, CGInterpolationQuality};
use objc2_core_ml::{
    MLArrayBatchProvider, MLBatchProvider, MLComputeUnits, MLDictionaryFeatureProvider, MLFeatureProvider, MLFeatureValue, MLModel, MLModelConfiguration,
    MLMultiArray, MLMultiArrayDataType,
};
use objc2_core_video::{
    CVPixelBuffer, CVPixelBufferCreate, CVPixelBufferGetBaseAddress, CVPixelBufferGetBytesPerRow, CVPixelBufferLockBaseAddress, CVPixelBufferLockFlags,
    CVPixelBufferUnlockBaseAddress, kCVPixelFormatType_32BGRA,
};
use objc2_foundation::{NSArray, NSDictionary, NSString, NSURL};
use objc2_image_io::{
    CGImageDestination, CGImageSource, kCGImageDestinationLossyCompressionQuality, kCGImageSourceCreateThumbnailFromImageAlways,
    kCGImageSourceCreateThumbnailFromImageIfAbsent, kCGImageSourceCreateThumbnailWithTransform, kCGImageSourceShouldCache,
    kCGImageSourceShouldCacheImmediately, kCGImageSourceThumbnailMaxPixelSize,
};
use std::path::Path;
use std::ptr::NonNull;

/// SigLIP 2 B/16-256's side
pub const SIDE: usize = 256;

/// A picture ready for the model: its 256 x 256 pixel buffer and its own size
pub struct Ready {
    pub item: i64,
    pub t: f64,
    pub px: CFRetained<CVPixelBuffer>,
    pub w: u32,
    pub h: u32,
}
// a pixel buffer is a reference-counted CF object; it moves between the decoding and embedding threads, used by one
// at a time
unsafe impl Send for Ready {}

fn dict(pairs: &[(&CFString, &CFType)]) -> CFRetained<CFDictionary<CFString, CFType>> {
    let (k, v): (Vec<&CFString>, Vec<&CFType>) = pairs.iter().copied().unzip();
    CFDictionary::from_slices(&k, &v)
}

fn thumbnail(src: &CGImageSource, max: i32, always: bool) -> Option<CFRetained<CGImage>> {
    let n = CFNumber::new_i32(max);
    let yes = CFBoolean::new(true);
    let from = if always { unsafe { kCGImageSourceCreateThumbnailFromImageAlways } } else { unsafe { kCGImageSourceCreateThumbnailFromImageIfAbsent } };
    let d = dict(&[
        (from, yes.as_ref()),
        (unsafe { kCGImageSourceThumbnailMaxPixelSize }, n.as_ref()),
        (unsafe { kCGImageSourceCreateThumbnailWithTransform }, yes.as_ref()),
        (unsafe { kCGImageSourceShouldCacheImmediately }, yes.as_ref()),
    ]);
    unsafe { src.thumbnail_at_index(0, Some(d.as_opaque())) }
}

/// A picture file's small copy at `SIDE`, its thumbnail written to `thumb`: the embedded preview when it's at least
/// `SIDE` on its long side, else the file decoded at a reduced size
pub fn picture(path: &Path, thumb: Option<&Path>) -> Result<(CFRetained<CVPixelBuffer>, u32, u32), String> {
    let url = CFURL::from_file_path(path).ok_or("not a file path")?;
    let no = CFBoolean::new(false);
    let opts = dict(&[(unsafe { kCGImageSourceShouldCache }, no.as_ref())]);
    let src = unsafe { CGImageSource::with_url(&url, Some(opts.as_opaque())) }.ok_or("ImageIO can't read it")?;
    let side = SIDE as i32;
    let img = thumbnail(&src, side, false)
        .filter(|i| CGImage::width(Some(i)).max(CGImage::height(Some(i))) >= SIDE)
        .or_else(|| thumbnail(&src, side, true))
        .ok_or("no picture in it")?;
    let (w, h) = (CGImage::width(Some(&img)) as u32, CGImage::height(Some(&img)) as u32);
    if let Some(t) = thumb {
        let _ = write_jpeg(&img, t);
    }
    Ok((squash(&img)?, w, h))
}

/// The picture drawn into a SIDE x SIDE BGRA pixel buffer, stretched to the square
pub fn squash(img: &CGImage) -> Result<CFRetained<CVPixelBuffer>, String> {
    let mut raw: *mut CVPixelBuffer = std::ptr::null_mut();
    let r = unsafe { CVPixelBufferCreate(None, SIDE, SIDE, kCVPixelFormatType_32BGRA, None, NonNull::new(&mut raw).unwrap()) };
    if r != 0 || raw.is_null() {
        return Err(format!("CVPixelBufferCreate: {r}"));
    }
    let px = unsafe { CFRetained::from_raw(NonNull::new(raw).unwrap()) };
    unsafe { CVPixelBufferLockBaseAddress(&px, CVPixelBufferLockFlags(0)) };
    let base = CVPixelBufferGetBaseAddress(&px);
    let row = CVPixelBufferGetBytesPerRow(&px);
    let space = CGColorSpace::new_device_rgb();
    // premultiplied first, little-endian: BGRA in memory
    let info = 2u32 | (2 << 12);
    let ctx = unsafe { CGBitmapContextCreate(base, SIDE, SIDE, 8, row, space.as_deref(), info) };
    let drawn = ctx.map(|c| {
        CGContext::set_interpolation_quality(Some(&c), CGInterpolationQuality::Medium);
        CGContext::draw_image(Some(&c), objc2_core_foundation::CGRect::new(objc2_core_foundation::CGPoint::new(0.0, 0.0), objc2_core_foundation::CGSize::new(SIDE as f64, SIDE as f64)), Some(img));
    });
    unsafe { CVPixelBufferUnlockBaseAddress(&px, CVPixelBufferLockFlags(0)) };
    drawn.ok_or("no bitmap context")?;
    Ok(px)
}

/// A JPEG of the picture at quality 0.7
pub fn write_jpeg(img: &CGImage, to: &Path) -> Result<(), String> {
    if let Some(d) = to.parent() {
        std::fs::create_dir_all(d).map_err(|e| e.to_string())?;
    }
    let url = CFURL::from_file_path(to).ok_or("not a file path")?;
    let kind = CFString::from_static_str("public.jpeg");
    let dest = unsafe { CGImageDestination::with_url(&url, &kind, 1, None) }.ok_or("ImageIO can't write there")?;
    let q = CFNumber::new_f64(0.7);
    let props = dict(&[(unsafe { kCGImageDestinationLossyCompressionQuality }, q.as_ref())]);
    unsafe { dest.add_image(img, Some(props.as_opaque())) };
    if unsafe { dest.finalize() } { Ok(()) } else { Err("the JPEG wasn't written".into()) }
}

/// SigLIP 2 B/16-256's image tower on Core ML
pub struct Model(Retained<MLModel>);
unsafe impl Send for Model {}

/// Whether the compiled copy is older than the package: a package's Manifest.json is written whenever it is saved or
/// copied in, and the compiled folder when it was compiled. No compiled copy, or no time to read, compiles again.
fn stale(compiled: &Path, package: &Path) -> bool {
    let (Ok(c), Ok(p)) = (std::fs::metadata(compiled), std::fs::metadata(package.join("Manifest.json"))) else { return true };
    match (c.modified(), p.modified()) {
        (Ok(c), Ok(p)) => c < p,
        _ => true,
    }
}

impl Model {
    /// The package compiled once into ~/Library/Caches/raea-index, loaded on the CPU and Neural Engine
    pub fn load(package: &Path) -> Result<Model, String> {
        if !package.exists() {
            return Err(format!("{}: not installed", package.display()));
        }
        let caches = raea_io_home::user_home().unwrap_or_default().join("Library/Caches/raea-index");
        std::fs::create_dir_all(&caches).map_err(|e| e.to_string())?;
        let name = package.file_stem().and_then(|s| s.to_str()).unwrap_or("model");
        let compiled = caches.join(format!("{name}.mlmodelc"));
        if stale(&compiled, package) {
            let url = NSURL::from_directory_path(package).ok_or("bad package path")?;
            #[allow(deprecated)]
            let fresh = unsafe { MLModel::compileModelAtURL_error(&url) }.map_err(|e| format!("compiling {name}: {}", e.localizedDescription()))?;
            let from = fresh.to_file_path().ok_or("no compiled path")?;
            let _ = std::fs::remove_dir_all(&compiled);
            std::fs::rename(&from, &compiled).map_err(|e| e.to_string())?;
        }
        let config = unsafe { MLModelConfiguration::new() };
        let units = match std::env::var("RAEA_COREML_UNITS").as_deref() {
            // to measure one against another; the default is the only right one on an M5 Pro
            Ok("cpu") => MLComputeUnits::CPUOnly,
            Ok("all") => MLComputeUnits::All,
            _ => MLComputeUnits::CPUAndNeuralEngine,
        };
        unsafe { config.setComputeUnits(units) };
        let url = NSURL::from_directory_path(&compiled).ok_or("bad compiled path")?;
        let m = unsafe { MLModel::modelWithContentsOfURL_configuration_error(&url, &config) }.map_err(|e| format!("loading {name}: {}", e.localizedDescription()))?;
        Ok(Model(m))
    }

    /// Each picture's embedding, L2-normalised, one batch through the model
    pub fn embed(&self, batch: &[&CVPixelBuffer]) -> Result<Vec<Vec<f32>>, String> {
        let key = NSString::from_str("image");
        let mut provs: Vec<Retained<ProtocolObject<dyn MLFeatureProvider>>> = Vec::with_capacity(batch.len());
        for px in batch {
            let v = unsafe { MLFeatureValue::featureValueWithPixelBuffer(px) };
            let d: Retained<NSDictionary<NSString, AnyObject>> = NSDictionary::from_slices(&[&*key], &[v.as_ref() as &AnyObject]);
            let p = unsafe { MLDictionaryFeatureProvider::initWithDictionary_error(MLDictionaryFeatureProvider::alloc(), &d) }.map_err(|e| format!("inputs: {}", e.localizedDescription()))?;
            provs.push(ProtocolObject::from_retained(p));
        }
        let arr = NSArray::from_retained_slice(&provs);
        let bp = unsafe { MLArrayBatchProvider::initWithFeatureProviderArray(MLArrayBatchProvider::alloc(), &arr) };
        let out = unsafe { self.0.predictionsFromBatch_error(ProtocolObject::from_ref(&*bp)) }.map_err(|e| format!("prediction: {}", e.localizedDescription()))?;
        let name = NSString::from_str("embedding");
        let n = unsafe { out.count() } as usize;
        let mut vs = Vec::with_capacity(n);
        for i in 0..n {
            let f = unsafe { out.featuresAtIndex(i as isize) };
            let v = unsafe { f.featureValueForName(&name) }.ok_or("no embedding output")?;
            let a: Retained<MLMultiArray> = unsafe { v.multiArrayValue() }.ok_or("embedding isn't an array")?;
            vs.push(floats(&a)?);
        }
        Ok(vs)
    }
}

fn floats(a: &MLMultiArray) -> Result<Vec<f32>, String> {
    let shape: Vec<usize> = unsafe { a.shape() }.iter().map(|n| n.integerValue() as usize).collect();
    let strides: Vec<usize> = unsafe { a.strides() }.iter().map(|n| n.integerValue() as usize).collect();
    let dim = *shape.last().ok_or("empty shape")?;
    let step = *strides.last().ok_or("empty strides")?;
    let kind = unsafe { a.dataType() };
    #[allow(deprecated)]
    let base = unsafe { a.dataPointer() }.as_ptr();
    let mut v: Vec<f32> = (0..dim)
        .map(|i| unsafe {
            if kind == MLMultiArrayDataType::Float16 {
                half::f16::from_bits(*(base as *const u16).add(i * step)).to_f32()
            } else {
                *(base as *const f32).add(i * step)
            }
        })
        .collect();
    crate::normalise(&mut v);
    Ok(v)
}
