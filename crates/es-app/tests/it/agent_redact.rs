use es_app::agent_redact::redact_secrets;
use es_app::errors::Error;
use es_geom::Rect;
use es_ocr::{Recognized, Word};
use image::{Rgba, RgbaImage};

fn word(text: &str, x: f64, y: f64, w: f64, line: u32) -> Word {
    Word {
        text: text.into(),
        rect: Rect::new(x, y, x + w, y + 12.0),
        confidence: 90.0,
        line,
    }
}

fn screen() -> RgbaImage {
    RgbaImage::from_fn(400, 200, |x, y| {
        let v = if (x / 2 + y / 2) % 2 == 0 { 20 } else { 235 };
        Rgba([v, v, v, 255])
    })
}

fn reading(
    words: Vec<Word>,
) -> impl Fn(&RgbaImage, &es_ocr::OcrOptions) -> Result<Recognized, es_ocr::Error> {
    move |_, _| {
        Ok(Recognized {
            words: words.clone(),
            ..Recognized::default()
        })
    }
}

fn unchanged(a: &RgbaImage, b: &RgbaImage, r: Rect) -> bool {
    (r.x0 as u32..r.x1 as u32)
        .all(|x| (r.y0 as u32..r.y1 as u32).all(|y| a.get_pixel(x, y) == b.get_pixel(x, y)))
}

#[test]
fn an_api_key_and_a_password_are_covered_and_the_rest_of_the_screen_is_not() {
    let words = vec![
        word("api_key:", 20.0, 20.0, 60.0, 0),
        word("sk_live_4eC39HqLyjWDarjtT1zdp7dc", 90.0, 20.0, 200.0, 0),
        word("password:", 20.0, 80.0, 70.0, 1),
        word("hunter2horse", 100.0, 80.0, 90.0, 1),
        word("Welcome", 20.0, 140.0, 60.0, 2),
        word("back", 90.0, 140.0, 30.0, 2),
    ];
    let base = screen();
    let done = redact_secrets(&base, &["eng".into()], &reading(words)).unwrap();
    assert_eq!(done.regions, 2);
    assert_eq!(done.image.dimensions(), base.dimensions());
    let key = Rect::new(100.0, 20.0, 280.0, 32.0);
    let password = Rect::new(105.0, 80.0, 185.0, 92.0);
    assert!(!unchanged(&base, &done.image, key), "the key");
    assert!(!unchanged(&base, &done.image, password), "the password");
    let plain = Rect::new(0.0, 130.0, 400.0, 200.0);
    assert!(unchanged(&base, &done.image, plain), "plain text stays");
}

#[test]
fn a_screen_with_nothing_secret_comes_back_as_it_was() {
    let base = screen();
    let words = vec![
        word("Welcome", 20.0, 20.0, 60.0, 0),
        word("back", 90.0, 20.0, 30.0, 0),
    ];
    let done = redact_secrets(&base, &["eng".into()], &reading(words)).unwrap();
    assert_eq!(done.regions, 0);
    assert_eq!(done.image, base);
}

#[test]
fn a_missing_tesseract_is_reported_not_swallowed() {
    let missing = |_: &RgbaImage, _: &es_ocr::OcrOptions| Err(es_ocr::Error::NotInstalled);
    let err = redact_secrets(&screen(), &["eng".into()], &missing).unwrap_err();
    assert!(matches!(err, Error::TesseractMissing), "{err:?}");
}
