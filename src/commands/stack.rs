// There are multiple ways to use modules from other parts of your crate:

// Method 1: Import specific items from a module
use crate::calibration;
use crate::image;

// Method 2: Import the entire module and use with path
// (Uncomment below to use this approach instead)
// use crate::image;

pub fn run_stack_command(
    lights_folder: String,
    darks_folder: Option<String>,
    flats_folder: Option<String>,
    bias_folder: Option<String>,
    output_folder: String,
    threads: Option<usize>,
) {
    println!("Running stack command with the following parameters:");
    println!("Lights folder: {}", lights_folder);
    println!("Darks folder: {:?}", darks_folder);
    println!("Flats folder: {:?}", flats_folder);
    println!("Bias folder: {:?}", bias_folder);
    println!("Output folder: {}", output_folder);
    println!("Threads: {:?}", threads);

    let result = image::FitsImage::from_folder(&lights_folder, image::FrameType::Light);

    // Check if the result is an error
    if let Err(e) = result {
        eprintln!("Error reading lights folder: {}", e);
        return;
    }

    println!("Successfully read lights folder.");

    // Unwrap the result to get the FitsImage
    // This is safe because we already checked for errors
    let fits_images = result.unwrap();

    println!("Number of images read: {}", fits_images.len());

    // Stack the images
    let stacked_image = calibration::average(&fits_images);

    // Check if the stacking was successful
    if let Err(e) = stacked_image {
        eprintln!("Error stacking images: {}", e);
        return;
    }

    // Unwrap the result to get the stacked image
    let stacked_image = stacked_image.unwrap();

    println!("Successfully stacked images.");

    let image_statistics = stacked_image.calculate_statistics();

    println!("Stacked image statistics:");
    println!("Mean: {}", image_statistics.mean);
    println!("Median: {}", image_statistics.median);
    println!("Standard Deviation: {}", image_statistics.std_dev);
    println!("Minimum: {}", image_statistics.min);
    println!("Maximum: {}", image_statistics.max);

    // Save the stacked image
    let output_path = format!("{}/stacked_image.fits", output_folder);
    stacked_image.to_file(&output_path).unwrap_or_else(|e| {
        eprintln!("Error saving stacked image: {}", e);
    });

    println!("Stacked image saved to: {}", output_path);
}
