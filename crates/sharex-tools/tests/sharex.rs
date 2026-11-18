//! The tool engines against outputs ShareX's own code wrote (a small set from
//! tools/tools-harness, collected by its make_fixtures.py). Everything here is
//! pure computation, so it runs on every platform; the full comparison is the
//! harness's validate example.

use std::path::PathBuf;

use sharex_tools::imaging::{self, Bitmap, BmpBitDepth, GifQuality, JpegDownsample, PngBitDepth};
use sharex_tools::{combiner, converter, icon, metadata, monitor_test, qr, resizer, thumbnailer};

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures").join(name)
}

fn input(name: &str) -> String {
    fixture(&format!("in-{name}")).to_string_lossy().into_owned()
}

fn expected(name: &str) -> Vec<u8> {
    std::fs::read(fixture(&format!("sharex-{name}"))).unwrap_or_else(|e| panic!("{name}: {e}"))
}

/// A zlib-compressed `.sxrb` (premultiplied BGRA) as a premultiplied RGBA bitmap.
fn expected_bitmap(name: &str) -> Bitmap {
    use std::io::Read;
    let z = expected(&format!("{name}.sxrb.z"));
    let mut d = Vec::new();
    flate2::read::ZlibDecoder::new(&z[..]).read_to_end(&mut d).unwrap();
    assert_eq!(&d[..4], b"SXRB");
    let w = u32::from_le_bytes(d[4..8].try_into().unwrap());
    let h = u32::from_le_bytes(d[8..12].try_into().unwrap());
    let mut px = d[12..].to_vec();
    for p in px.chunks_exact_mut(4) {
        p.swap(0, 2);
    }
    Bitmap::from_premultiplied(w, h, px)
}

fn assert_same_bitmap(ours: &Bitmap, theirs: &Bitmap, what: &str) {
    assert_eq!((ours.width(), ours.height()), (theirs.width(), theirs.height()), "{what}: size");
    let diff = ours.data().iter().zip(theirs.data()).position(|(a, b)| a != b);
    assert!(diff.is_none(), "{what}: first difference at byte {diff:?}");
}

#[test]
fn jpeg_thumbnail_is_byte_identical() {
    // libjpeg-turbo decode, EXIF rotation, Mitchell crop, libjpeg-turbo encode.
    let src = imaging::load_image(input("exif6.jpg")).unwrap();
    let thumb = thumbnailer::create(&src, 64, 32).unwrap();
    let jpg = imaging::save_jpeg(&thumb, 90, JpegDownsample::Downsample420);
    assert_eq!(jpg, expected("thumb-exif6-64x32-q90-exif6_th.jpg"));
}

#[test]
fn jpeg_decoding_matches_libjpeg_turbo() {
    let ours = converter::load_image(&input("photo_q90.jpg")).unwrap();
    assert_same_bitmap(&ours, &expected_bitmap("conv-photo_q90_jpg-png-Automatic.loaded"), "photo_q90.jpg");
}

#[test]
fn webp_decoding_premultiplies_like_libwebp() {
    let ours = converter::load_image(&input("lossless.webp")).unwrap();
    assert_same_bitmap(&ours, &expected_bitmap("conv-lossless_webp-png-Automatic.loaded"), "lossless.webp");
}

#[test]
fn converter_outputs_are_byte_identical() {
    let photo = converter::load_image(&input("photo_q90.jpg")).unwrap();
    let o = converter::ConvertOptions {
        format: converter::OutputFormat::Jpeg,
        quality: 40,
        jpeg_subsampling: JpegDownsample::Downsample422,
        ..Default::default()
    };
    assert_eq!(converter::encode(&photo, &o), expected("conv-photo_q90_jpg-jpeg-Downsample422-q40.jpg"), "JPEG with ICC");
    let alpha = converter::load_image(&input("alpha_64x48.png")).unwrap();
    let bmp = converter::ConvertOptions {
        format: converter::OutputFormat::Bmp,
        bmp_bit_depth: BmpBitDepth::Bit32,
        background: [30, 144, 255, 255],
        ..Default::default()
    };
    assert_eq!(converter::encode(&alpha, &bmp), expected("conv-alpha_64x48_png-bmp-Bit32.bmp"), "BMP");
    let gif = converter::ConvertOptions { format: converter::OutputFormat::Gif, gif_quality: GifQuality::Adaptive, ..Default::default() };
    assert_eq!(converter::encode(&alpha, &gif), expected("conv-alpha_64x48_png-gif-Adaptive.gif"), "adaptive GIF");
    let _ = PngBitDepth::Automatic;
}

#[test]
fn gif_maker_is_byte_identical() {
    let files = [input("opaque_97x61.png"), input("odd_33x77.png")];
    let gif = sharex_tools::video::gif_maker::create_bytes(&files, 100, true, 0).unwrap();
    assert_eq!(gif, expected("gifmaker-two-d100-loop-r0.gif"));
}

#[test]
fn icons_are_byte_identical() {
    let alpha = icon::load_image(&input("alpha_64x48.png")).unwrap();
    assert_eq!(icon::convert(&alpha, &[16], icon::IconBitDepth::Palette8).unwrap(), expected("icon-alpha_64x48-Palette8-16.ico"));
    let opaque = icon::load_image(&input("opaque_97x61.png")).unwrap();
    assert_eq!(
        icon::convert(&opaque, &[48, 16, 32, 16], icon::IconBitDepth::TrueColor32).unwrap(),
        expected("icon-opaque_97x61-TrueColor32-48_16_32_16.ico")
    );
}

#[test]
fn qr_codes_are_identical() {
    let text = "é ü 日本語 😀";
    assert_same_bitmap(&qr::generate(text, 100).unwrap(), &expected_bitmap("qr-utf8-100"), "QR bitmap");
    assert_eq!(qr::generate_svg_file(text, 100).unwrap(), expected("qr-utf8-100.svg"), "QR SVG");
    assert_eq!(qr::scan(&qr::generate(text, 100).unwrap(), false), Some(vec![text.to_string()]));
}

#[test]
fn combiner_and_resizer_match() {
    let o = combiner::ImageCombinerOptions {
        orientation: combiner::Orientation::Horizontal,
        alignment: combiner::Alignment::Center,
        space: 7,
        wrap_after: 2,
        auto_fill_background: false,
    };
    let files = [input("opaque_97x61.png"), input("alpha_64x48.png"), input("odd_33x77.png")];
    assert_same_bitmap(&combiner::combine_files(&files, &o).unwrap(), &expected_bitmap("combine-mix-H-Center-s7-w2-nofill"), "combined");
    let src = imaging::load_image(input("alpha_64x48.png")).unwrap();
    let fit = resizer::resize(&src, 50, 120, resizer::ResizeMode::Fit).unwrap();
    assert_same_bitmap(&fit, &expected_bitmap("resize-alpha_64x48-Fit-50x120"), "resized");
}

fn decoded(bytes: &[u8]) -> Vec<u8> {
    image::load_from_memory(bytes).unwrap().to_rgba8().into_raw()
}

#[test]
fn monitor_test_patterns_match() {
    let gradient = monitor_test::MonitorTest {
        mode: monitor_test::Mode::Gradient,
        gradient_direction: monitor_test::GradientDirection::ForwardDiagonal,
        gradient_start: [255, 255, 255, 255],
        gradient_end: [0, 0, 0, 255],
        ..Default::default()
    };
    let ours = imaging::encode_png(&gradient.render(257, 129, 0.0));
    assert_eq!(decoded(&ours), decoded(&expected("monitor-gradient-ForwardDiagonal-2550.png")), "gradient");
    let motion = monitor_test::MonitorTest { mode: monitor_test::Mode::Motion, ..Default::default() };
    let ours = imaging::encode_png(&motion.render(301, 167, 37.25));
    assert_eq!(decoded(&ours), decoded(&expected("monitor-motion-VerticalBars-37.25.png")), "motion bars");
}

#[test]
fn gif_trimmer_reencode_is_byte_identical() {
    use sharex_tools::video::gif_trimmer::{AnimatedGifTrimmerDocument, CropRect, ExportResult};
    let data = std::fs::read(input("disp1.gif")).unwrap();
    let doc = AnimatedGifTrimmerDocument::open(data).unwrap();
    assert_same_bitmap(&doc.image.frame(4), &expected_bitmap("gifreencode-disp1_gif-frame4"), "frame 4");
    let full = CropRect { x: 0, y: 0, width: 48, height: 36 };
    let cases = [
        (1, 7, true, full, (48, 36), "export3"),
        (0, 7, false, CropRect { x: 5, y: 3, width: 36, height: 27 }, (36, 27), "export10"),
        (0, 5, true, CropRect { x: 7, y: 2, width: 24, height: 18 }, (16, 12), "export12"),
    ];
    for (first, end, reencode, crop, size, name) in cases {
        let ExportResult::Written(ours) = doc.export_bytes(first, end, reencode, crop, size, None).unwrap() else {
            panic!("{name}: not written");
        };
        assert_eq!(ours, expected(&format!("gifreencode-disp1_gif-{name}.gif")), "{name}");
    }
}

#[test]
fn gif_lossless_trim_is_byte_identical() {
    let data = std::fs::read(input("trim.gif")).unwrap();
    let t = sharex_tools::video::gif_trimmer::GifLosslessTrimmer::parse(&data).unwrap();
    assert_eq!(t.copy(&data, 1, 3).unwrap(), expected("giftrim-trim_gif-1-3.gif"));
}

#[test]
fn metadata_reads_like_sharex() {
    for name in ["jpg", "png"] {
        let path = input(&format!("meta-full.{name}"));
        let theirs: serde_json::Value = serde_json::from_slice(&expected(&format!("meta-full_{name}.json"))).unwrap();
        let ours: Vec<serde_json::Value> = metadata::read(&path)
            .unwrap()
            .into_iter()
            .filter(|m| m.group != "File System" && m.tag != "File Name")
            .map(|m| serde_json::json!([m.group, m.tag, m.value]))
            .collect();
        let theirs: Vec<serde_json::Value> =
            theirs["read"].as_array().unwrap().iter().filter(|x| x[1] != "File Name").cloned().collect();
        assert_eq!(ours, theirs, "{name}");
    }
}

#[test]
fn ffmpeg_command_lines_match() {
    use sharex_tools::video::{converter as vc, trimmer};
    let theirs: serde_json::Value = serde_json::from_slice(&expected("vargs-nofolder.json")).unwrap();
    let o = vc::VideoConverterOptions {
        input_file_path: Some("a.mp4".into()),
        output_file_name: Some("b".into()),
        video_codec: vc::Codec::Vp9,
        video_quality: 30,
        ..Default::default()
    };
    assert_eq!(theirs["optionsArgs"], o.arguments());
    let theirs: serde_json::Value = serde_json::from_slice(&expected("vtrim-1.5-4.25-precise-mp4.json")).unwrap();
    let ours = trimmer::build_trim_arguments("in clip.mp4", "out clip.mp4", 1.5, 4.25, true);
    assert_eq!(theirs, serde_json::json!(ours));
}
