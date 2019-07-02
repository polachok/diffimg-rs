extern crate clap;
extern crate image;

use std::path::Path;

use clap::ArgMatches;
use image::{DynamicImage, GenericImage, GenericImageView};

#[derive(Debug)]
pub struct Config<'a> {
    pub image1: &'a str,
    pub image2: &'a str,
    pub filename: Option<&'a str>,
}

impl<'a> Config<'a> {
    pub fn from_clap_matches(matches: &'a ArgMatches) -> Config<'a> {
        // unwrap() should be safe here because clap does argument validation
        let image1 = matches.value_of("image1").unwrap();
        let image2 = matches.value_of("image2").unwrap();
        let filename = matches.value_of("filename");

        Config {
            image1,
            image2,
            filename,
        }
    }
}

/// abs(x - y) for u8
fn abs_diff(x: u8, y: u8) -> u8 {
    if x > y {
        return x - y;
    }
    return y - x;
}

#[test]
fn test_abs_diff() {
    assert_eq!(abs_diff(5, 8), 3);
    assert_eq!(abs_diff(8, 5), 3);
    assert_eq!(abs_diff(11, 11), 0);
    assert_eq!(abs_diff(0, 255), 255);
}

/// Return the image from the file path, or throw an error
fn safe_load_image(raw_path: &str) -> Result<DynamicImage, String> {
    let path = &Path::new(raw_path);
    if !Path::exists(path) {
        return Err(format!("Path \"{}\" does not exist", raw_path));
    }

    match image::open(path) {
        Ok(image) => Ok(image),
        Err(msg) => Err(format!("{:?}", msg)),
    }
}

/// Check if two images are the same size and color mode
fn validate_image_compatibility(
    image1: &DynamicImage,
    image2: &DynamicImage,
) -> Result<(), String> {
    if image1.dimensions() != image2.dimensions() {
        return Err("images must have the same dimensions".to_string());
    }
    if image1.color() != image2.color() {
        return Err("images must have the same color mode".to_string());
    }

    Ok(())
}

/// Return a difference ratio between 0 and 1 for the two images
pub fn calculate_diff_ratio(image1: DynamicImage, image2: DynamicImage) -> f64 {
    use std::arch::x86_64::*;
    // All color types wrap an 8-bit value for each channel
    let max_val = u64::pow(2, 8) - 1;
    let mut diffsum: u64 = 0;
    let image1 = image1.raw_pixels();
    let image2 = image2.raw_pixels();
    let len = image1.len().min(image2.len());

    for i in (0..len).step_by(32) {
        let a = &image1[i..i+32];
        let b = &image2[i..i+32];
        let a = unsafe { _mm256_loadu_si256(a.as_ptr() as *const _) };
        let b = unsafe { _mm256_loadu_si256(b.as_ptr() as *const _) };
        let result = unsafe { _mm256_sad_epu8(a, b) };
        let (a, b, c, d): (u64, u64, u64, u64) = unsafe { std::mem::transmute(result) };
        diffsum += a + b + c + d;
    }
    let total_possible = max_val * image1.len() as u64;
    let ratio = diffsum as f64 / total_possible as f64;

    ratio
}

/// Create an image that is the difference of the two images given, and write to the given filename
pub fn create_diff_image(
    image1: DynamicImage,
    image2: DynamicImage,
    filename: &str,
) -> Result<(), String> {
    let w = image1.width();
    let h = image1.height();

    let mut diff = match image1.color() {
        image::ColorType::RGB(_) => image::DynamicImage::new_rgb8(w, h),
        image::ColorType::RGBA(_) => image::DynamicImage::new_rgba8(w, h),
        _ => return Err(format!("color mode {:?} not yet supported", image1.color())),
    };

    for x in 0..w {
        for y in 0..h {
            let mut rgba = [0; 4];
            for c in 0..4 {
                rgba[c] = abs_diff(
                    image1.get_pixel(x, y).data[c],
                    image2.get_pixel(x, y).data[c],
                );
            }
            let new_pix = image::Pixel::from_slice(&rgba);
            diff.put_pixel(x, y, *new_pix);
        }
    }

    if let Err(msg) = diff.save(filename) {
        return Err(msg.to_string());
    }

    Ok(())
}

/// Run the appropriate diffing process given the configuration settings
pub fn run(config: Config) -> Result<(), String> {
    let image1 = safe_load_image(&config.image1)?;
    let image2 = safe_load_image(&config.image2)?;
    validate_image_compatibility(&image1, &image2)?;

    match config.filename {
        Some(filename) => match create_diff_image(image1, image2, filename) {
            Ok(_) => {
                println!("Wrote diff image to {}", filename);
                Ok(())
            }
            Err(msg) => Err(msg),
        },
        None => {
            let ratio = calculate_diff_ratio(image1, image2);
            println!("{}", ratio);
            return Ok(());
        }
    }
}
