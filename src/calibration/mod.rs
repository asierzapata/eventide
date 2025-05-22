use crate::image::{FitsImage, ImageError};

/// Combine multiple FITS images by calculating the average value for each pixel
pub fn average(images: &[FitsImage]) -> Result<FitsImage, ImageError> {
    if images.is_empty() {
        return Err(ImageError::FormatError(
            "No images provided for averaging".to_string(),
        ));
    }

    // Use the first image as a template
    let first = &images[0];
    let (width, height) = first.dimensions();

    // Check that all images have the same dimensions
    for img in images.iter().skip(1) {
        if img.dimensions() != (width, height) {
            return Err(ImageError::DimensionError(
                "All images must have the same dimensions for averaging".to_string(),
            ));
        }
    }

    println!("Image dimensions: {} x {}", width, height);
    println!("Creating average image...");

    // Create a new image to hold the average
    let mut result = FitsImage::new(width, height);

    // Copy metadata from the first image
    result.metadata = first.metadata.clone();
    result.frame_type = first.frame_type;

    println!("Calculating average pixel values in parallel...");

    use rayon::prelude::*;

    // Calculate averages in parallel
    let pixel_values: Vec<((usize, usize), f32)> = (0..height)
        .into_par_iter()
        .flat_map(|y| {
            let mut row_results = Vec::with_capacity(width);
            for x in 0..width {
                let sum: f32 = images.iter().map(|img| img.data[[y, x]]).sum();
                let avg = sum / images.len() as f32;
                row_results.push(((y, x), avg));
            }
            println!("Processed row {} of {}", y, height);
            row_results
        })
        .collect();

    // Fill the result array
    let result_data = result.data_mut();
    for ((y, x), avg) in pixel_values {
        result_data[[y, x]] = avg;
    }

    Ok(result)
}

/// Combine multiple FITS images by calculating the median value for each pixel
pub fn median(images: &[FitsImage]) -> Result<FitsImage, ImageError> {
    if images.is_empty() {
        return Err(ImageError::FormatError(
            "No images provided for median".to_string(),
        ));
    }

    // Use the first image as a template
    let first = &images[0];
    let (width, height) = first.dimensions();

    // Check that all images have the same dimensions
    for img in images.iter().skip(1) {
        if img.dimensions() != (width, height) {
            return Err(ImageError::DimensionError(
                "All images must have the same dimensions for median".to_string(),
            ));
        }
    }

    // Create a new image to hold the median
    let mut result = FitsImage::new(width, height);

    // Copy metadata from the first image
    result.metadata = first.metadata.clone();
    result.frame_type = first.frame_type;

    // Calculate the median pixel value for each position
    let result_data = result.data_mut();

    for y in 0..height {
        for x in 0..width {
            let mut values: Vec<f32> = images.iter().map(|img| img.data[[y, x]]).collect();

            values.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

            let median = if values.len() % 2 == 0 {
                let mid = values.len() / 2;
                (values[mid - 1] + values[mid]) / 2.0
            } else {
                values[values.len() / 2]
            };

            result_data[[y, x]] = median;
        }
    }

    Ok(result)
}

/// Apply sigma clipping to combine multiple FITS images
pub fn sigma_clipping(
    images: &[FitsImage],
    sigma: f32,
    iterations: usize,
) -> Result<FitsImage, ImageError> {
    if images.is_empty() {
        return Err(ImageError::FormatError(
            "No images provided for sigma clipping".to_string(),
        ));
    }

    // Use the first image as a template
    let first = &images[0];
    let (width, height) = first.dimensions();

    // Check that all images have the same dimensions
    for img in images.iter().skip(1) {
        if img.dimensions() != (width, height) {
            return Err(ImageError::DimensionError(
                "All images must have the same dimensions for sigma clipping".to_string(),
            ));
        }
    }

    // Create a new image to hold the result
    let mut result = FitsImage::new(width, height);

    // Copy metadata from the first image
    result.metadata = first.metadata.clone();
    result.frame_type = first.frame_type;

    // Apply sigma clipping for each pixel position
    let result_data = result.data_mut();

    for y in 0..height {
        for x in 0..width {
            // Get values for this pixel from all images
            let mut values: Vec<f32> = images.iter().map(|img| img.data[[y, x]]).collect();

            // Apply sigma clipping iterations
            for _ in 0..iterations {
                if values.len() <= 2 {
                    break;
                }

                // Calculate mean and standard deviation
                let mean: f32 = values.iter().sum::<f32>() / values.len() as f32;
                let variance: f32 =
                    values.iter().map(|&v| (v - mean).powi(2)).sum::<f32>() / values.len() as f32;
                let std_dev = variance.sqrt();

                // Reject outliers
                let lower_bound = mean - sigma * std_dev;
                let upper_bound = mean + sigma * std_dev;

                values.retain(|&v| v >= lower_bound && v <= upper_bound);
            }

            // Calculate mean of remaining values
            if values.is_empty() {
                result_data[[y, x]] = 0.0;
            } else {
                result_data[[y, x]] = values.iter().sum::<f32>() / values.len() as f32;
            }
        }
    }

    Ok(result)
}

// TODO: Implement the following functions
// /// Create a master dark frame from a list of dark frames
// pub fn create_master_dark(dark_frames: &[FitsImage]) -> Result<FitsImage, ImageError> {
//     // Use median stacking for dark frames
//     let mut master_dark = FitsImage::median(dark_frames)?;
//     master_dark.frame_type = FrameType::Dark;

//     // Update metadata
//     if let Some(first_exposure) = dark_frames.first().and_then(|f| f.metadata.exposure_time) {
//         master_dark.metadata.exposure_time = Some(first_exposure);
//     }

//     if let Some(first_temp) = dark_frames.first().and_then(|f| f.metadata.temperature) {
//         master_dark.metadata.temperature = Some(first_temp);
//     }

//     Ok(master_dark)
// }

// /// Create a master flat frame from a list of flat frames
// pub fn create_master_flat(flat_frames: &[FitsImage]) -> Result<FitsImage, ImageError> {
//     // Use average stacking for flat frames
//     let mut master_flat = FitsImage::average(flat_frames)?;
//     master_flat.frame_type = FrameType::Flat;

//     // Normalize the master flat
//     let stats = master_flat.calculate_statistics();
//     if stats.max > 0.0 {
//         let (width, height) = master_flat.dimensions();
//         for y in 0..height {
//             for x in 0..width {
//                 master_flat.data[[y, x]] /= stats.mean;
//             }
//         }
//     }

//     Ok(master_flat)
// }

// /// Create a master bias frame from a list of bias frames
// pub fn create_master_bias(bias_frames: &[FitsImage]) -> Result<FitsImage, ImageError> {
//     // Use median stacking for bias frames
//     let mut master_bias = FitsImage::median(bias_frames)?;
//     master_bias.frame_type = FrameType::Bias;

//     Ok(master_bias)
// }

// /// Calibrate a light frame using master dark and master flat frames
// pub fn calibrate(
//     &mut self,
//     master_dark: Option<&FitsImage>,
//     master_flat: Option<&FitsImage>,
// ) -> Result<(), ImageError> {
//     // Apply dark frame subtraction if provided
//     if let Some(dark) = master_dark {
//         self.subtract(dark)?;
//     }

//     // Apply flat field correction if provided
//     if let Some(flat) = master_flat {
//         self.divide(flat)?;
//     }

//     Ok(())
// }
