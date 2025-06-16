use std::error::Error;
use std::fmt;
use std::io;
use std::path::{Path, PathBuf};

use fitsio::FitsFile;
use fitsio::images::ImageDescription;
use fitsio::images::ImageType;
use ndarray::{ArrayD, IxDyn};

/// Possible pixel data types in FITS images
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PixelType {
    U8,
    U16,
    U32,
    I16,
    I32,
    F32,
    F64,
}

impl PixelType {
    /// Get the number of bytes used by this pixel type
    pub fn bytes_per_pixel(&self) -> usize {
        match self {
            PixelType::U8 => 1,
            PixelType::U16 => 2,
            PixelType::U32 => 4,
            PixelType::I16 => 2,
            PixelType::I32 => 4,
            PixelType::F32 => 4,
            PixelType::F64 => 8,
        }
    }
}

/// Metadata associated with a FITS image
#[derive(Debug, Clone)]
pub struct ImageMetadata {
    /// Dimensions of the image (width, height)
    pub dimensions: (usize, usize),
    /// Pixel type
    pub pixel_type: PixelType,
    /// Exposure time in seconds
    pub exposure_time: Option<f64>,
    /// Image temperature in degrees Celsius
    pub temperature: Option<f64>,
    /// ISO/Gain setting
    pub iso_gain: Option<u32>,
    /// Filter used (if any)
    pub filter: Option<String>,
    /// Original file path
    pub file_path: Option<PathBuf>,
    /// Additional key-value metadata
    pub extra: std::collections::HashMap<String, String>,
}

/// Image statistics
pub struct ImageStatistics {
    /// Minimum pixel value
    pub min: f32,
    /// Maximum pixel value
    pub max: f32,
    /// Mean pixel value
    pub mean: f32,
    /// Median pixel value
    pub median: f32,
    /// Standard deviation of pixel values
    pub std_dev: f32,
}

impl Default for ImageMetadata {
    fn default() -> Self {
        Self {
            dimensions: (0, 0),
            pixel_type: PixelType::U16, // Most common for astronomical images
            exposure_time: None,
            temperature: None,
            iso_gain: None,
            filter: None,
            file_path: None,
            extra: std::collections::HashMap::new(),
        }
    }
}

/// Calibration frame type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FrameType {
    Light,
    Dark,
    Flat,
    Bias,
    DarkFlat,
}

/// Error types for image operations
#[derive(Debug)]
pub enum ImageError {
    IoError(io::Error),
    FitsError(String),
    DimensionError(String),
    FormatError(String),
    UnsupportedOperation(String),
}

impl fmt::Display for ImageError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            ImageError::IoError(err) => write!(f, "IO error: {}", err),
            ImageError::FitsError(msg) => write!(f, "FITS error: {}", msg),
            ImageError::DimensionError(msg) => write!(f, "Dimension error: {}", msg),
            ImageError::FormatError(msg) => write!(f, "Format error: {}", msg),
            ImageError::UnsupportedOperation(msg) => write!(f, "Unsupported operation: {}", msg),
        }
    }
}

impl Error for ImageError {}

impl From<io::Error> for ImageError {
    fn from(err: io::Error) -> Self {
        ImageError::IoError(err)
    }
}

impl From<fitsio::errors::Error> for ImageError {
    fn from(err: fitsio::errors::Error) -> Self {
        ImageError::FitsError(err.to_string())
    }
}

/// Core FITS image struct
#[derive(Debug, Clone)]
pub struct FitsImage {
    /// Metadata for the image
    pub metadata: ImageMetadata,
    /// The actual pixel data
    pub data: ArrayD<f32>,
    /// The frame type
    pub frame_type: FrameType,
}

impl FitsImage {
    pub fn new(width: usize, height: usize) -> Self {
        let shape = IxDyn(&[height, width]);
        let data = ArrayD::<f32>::zeros(shape);

        Self {
            metadata: ImageMetadata::default(),
            data,
            frame_type: FrameType::Light,
        }
    }

    pub fn from_folder<P: AsRef<Path>>(
        path: P,
        frame_type: FrameType,
    ) -> Result<Vec<Self>, ImageError> {
        let path = path.as_ref();
        let mut images = Vec::new();

        // Iterate over all files in the directory
        for entry in std::fs::read_dir(path)? {
            let entry = entry?;
            let file_path = entry.path();

            // Check if the file is a FITS file
            if file_path
                .extension()
                .map_or(false, |ext| ext == "fits" || ext == "fit" || ext == "fts")
            {
                println!("Loading FITS file: {:?}", file_path);
                let image = FitsImage::from_file(&file_path, frame_type)?;
                println!("Loaded FITS file: {:?}", file_path);
                images.push(image);
            }
        }

        Ok(images)
    }

    /// Load a FITS image from a file
    pub fn from_file<P: AsRef<Path>>(path: P, frame_type: FrameType) -> Result<Self, ImageError> {
        let path = path.as_ref();
        let mut fitsfile = FitsFile::open(path)?;

        // Access the primary HDU (Header Data Unit)
        let hdu = fitsfile.primary_hdu()?;

        match &hdu.info {
            fitsio::hdu::HduInfo::ImageInfo { shape, image_type } => {
                // Check if the image is 2D
                if shape.len() != 2 {
                    return Err(ImageError::UnsupportedOperation(
                        "Only 2D images are supported".to_string(),
                    ));
                }

                let height = shape[0] as usize;
                let width = shape[1] as usize;

                // Initialize metadata
                let mut metadata = ImageMetadata {
                    dimensions: (width, height),
                    file_path: Some(path.to_owned()),
                    ..Default::default()
                };

                // Extract common FITS keywords
                if let Ok(exptime) = hdu.read_key::<f64>(&mut fitsfile, "EXPTIME") {
                    metadata.exposure_time = Some(exptime);
                }

                if let Ok(temp) = hdu.read_key::<f64>(&mut fitsfile, "CCD-TEMP") {
                    metadata.temperature = Some(temp);
                }

                if let Ok(filter) = hdu.read_key::<String>(&mut fitsfile, "FILTER") {
                    metadata.filter = Some(filter);
                }

                // Read the pixel data into an ndarray
                let data: ArrayD<f32> = match image_type {
                    fitsio::images::ImageType::Byte => {
                        metadata.pixel_type = PixelType::U8;
                        let pixels: Vec<u8> = hdu.read_image(&mut fitsfile)?;
                        ndarray::Array::<u8, _>::from_shape_vec(IxDyn(&[height, width]), pixels)
                            .map_err(|e| ImageError::DimensionError(e.to_string()))?
                            .mapv(|x| x as f32)
                            .into_dyn()
                    }
                    fitsio::images::ImageType::LongLong => {
                        metadata.pixel_type = PixelType::I32;
                        let pixels: Vec<i64> = hdu.read_image(&mut fitsfile)?;
                        ndarray::Array::<i64, _>::from_shape_vec(IxDyn(&[height, width]), pixels)
                            .map_err(|e| ImageError::DimensionError(e.to_string()))?
                            .mapv(|x| x as f32)
                            .into_dyn()
                    }
                    fitsio::images::ImageType::UnsignedByte => {
                        metadata.pixel_type = PixelType::U8;
                        let pixels: Vec<u8> = hdu.read_image(&mut fitsfile)?;
                        ndarray::Array::<u8, _>::from_shape_vec(IxDyn(&[height, width]), pixels)
                            .map_err(|e| ImageError::DimensionError(e.to_string()))?
                            .mapv(|x| x as f32)
                            .into_dyn()
                    }
                    fitsio::images::ImageType::UnsignedLong => {
                        metadata.pixel_type = PixelType::U32;
                        let pixels: Vec<u32> = hdu.read_image(&mut fitsfile)?;
                        ndarray::Array::<u32, _>::from_shape_vec(IxDyn(&[height, width]), pixels)
                            .map_err(|e| ImageError::DimensionError(e.to_string()))?
                            .mapv(|x| x as f32)
                            .into_dyn()
                    }
                    fitsio::images::ImageType::Double => {
                        metadata.pixel_type = PixelType::F64;
                        let pixels: Vec<f64> = hdu.read_image(&mut fitsfile)?;
                        ndarray::Array::<f64, _>::from_shape_vec(IxDyn(&[height, width]), pixels)
                            .map_err(|e| ImageError::DimensionError(e.to_string()))?
                            .mapv(|x| x as f32)
                            .into_dyn()
                    }
                    fitsio::images::ImageType::Float => {
                        metadata.pixel_type = PixelType::F32;
                        let pixels: Vec<f32> = hdu.read_image(&mut fitsfile)?;
                        ndarray::Array::<f32, _>::from_shape_vec(IxDyn(&[height, width]), pixels)
                            .map_err(|e| ImageError::DimensionError(e.to_string()))?
                            .into_dyn()
                    }
                    fitsio::images::ImageType::Short => {
                        metadata.pixel_type = PixelType::I16;
                        let pixels: Vec<i16> = hdu.read_image(&mut fitsfile)?;
                        ndarray::Array::<i16, _>::from_shape_vec(IxDyn(&[height, width]), pixels)
                            .map_err(|e| ImageError::DimensionError(e.to_string()))?
                            .mapv(|x| x as f32)
                            .into_dyn()
                    }
                    fitsio::images::ImageType::UnsignedShort => {
                        metadata.pixel_type = PixelType::U16;
                        let pixels: Vec<u16> = hdu.read_image(&mut fitsfile)?;
                        ndarray::Array::<u16, _>::from_shape_vec(IxDyn(&[height, width]), pixels)
                            .map_err(|e| ImageError::DimensionError(e.to_string()))?
                            .mapv(|x| x as f32)
                            .into_dyn()
                    }
                    _ => {
                        return Err(ImageError::UnsupportedOperation(
                            "Unsupported FITS data type".to_string(),
                        ));
                    }
                };

                // Determine frame type based on FITS header if available
                let frame_type =
                    if let Ok(frametype) = hdu.read_key::<String>(&mut fitsfile, "FRAME") {
                        match frametype.to_lowercase().as_str() {
                            "light" => FrameType::Light,
                            "dark" => FrameType::Dark,
                            "flat" => FrameType::Flat,
                            "bias" => FrameType::Bias,
                            "darkflat" => FrameType::DarkFlat,
                            _ => FrameType::Light, // Default to light frame
                        }
                    } else {
                        frame_type
                    };

                Ok(Self {
                    metadata,
                    data,
                    frame_type,
                })
            }
            _ => {
                return Err(ImageError::UnsupportedOperation(
                    "Only image HDUs are supported".to_string(),
                ));
            }
        }
    }

    /// Save the image to a FITS file
    pub fn to_file<P: AsRef<Path>>(&self, path: P) -> Result<(), ImageError> {
        let path = path.as_ref();

        // Create a new FITS file
        let description = ImageDescription {
            data_type: ImageType::Double,
            dimensions: &[self.metadata.dimensions.0, self.metadata.dimensions.1],
        };
        let mut fitsfile = FitsFile::create(path)
            .with_custom_primary(&description)
            .open()?;

        // Write metadata
        let hdu = fitsfile.primary_hdu()?;

        if let Some(exptime) = self.metadata.exposure_time {
            hdu.write_key(&mut fitsfile, "EXPTIME", exptime)?;
        }

        if let Some(temp) = self.metadata.temperature {
            hdu.write_key(&mut fitsfile, "CCD-TEMP", temp)?;
        }

        if let Some(ref filter) = self.metadata.filter {
            hdu.write_key(&mut fitsfile, "FILTER", filter.as_str())?;
        }

        // Write frame type
        match self.frame_type {
            FrameType::Light => hdu.write_key(&mut fitsfile, "FRAME", "LIGHT")?,
            FrameType::Dark => hdu.write_key(&mut fitsfile, "FRAME", "DARK")?,
            FrameType::Flat => hdu.write_key(&mut fitsfile, "FRAME", "FLAT")?,
            FrameType::Bias => hdu.write_key(&mut fitsfile, "FRAME", "BIAS")?,
            FrameType::DarkFlat => hdu.write_key(&mut fitsfile, "FRAME", "DARKFLAT")?,
        }

        // Write extra metadata
        for (key, value) in &self.metadata.extra {
            // FITS keys are limited to 8 characters
            let key = if key.len() > 8 { &key[0..8] } else { key };

            hdu.write_key(&mut fitsfile, key, value.as_str())?;
        }

        // Write the pixel data based on the original pixel type
        match self.metadata.pixel_type {
            PixelType::U8 => {
                let data: Vec<u8> = self.data.iter().map(|&x| x as u8).collect();
                hdu.write_image(&mut fitsfile, &data)?;
            }
            PixelType::I16 => {
                let data: Vec<i16> = self.data.iter().map(|&x| x as i16).collect();
                hdu.write_image(&mut fitsfile, &data)?;
            }
            PixelType::U16 => {
                let data: Vec<u16> = self.data.iter().map(|&x| x as u16).collect();
                hdu.write_image(&mut fitsfile, &data)?;
            }
            PixelType::U32 => {
                let data: Vec<u32> = self.data.iter().map(|&x| x as u32).collect();
                hdu.write_image(&mut fitsfile, &data)?;
            }
            PixelType::I32 => {
                let data: Vec<i32> = self.data.iter().map(|&x| x as i32).collect();
                hdu.write_image(&mut fitsfile, &data)?;
            }
            PixelType::F32 => {
                let data: Vec<f32> = self.data.iter().cloned().collect();
                hdu.write_image(&mut fitsfile, &data)?;
            }
            PixelType::F64 => {
                let data: Vec<f64> = self.data.iter().map(|&x| x as f64).collect();
                hdu.write_image(&mut fitsfile, &data)?;
            }
        }

        Ok(())
    }

    /// Get a reference to the image data
    pub fn data(&self) -> &ArrayD<f32> {
        &self.data
    }

    /// Get a mutable reference to the image data
    pub fn data_mut(&mut self) -> &mut ArrayD<f32> {
        &mut self.data
    }

    /// Get the dimensions of the image
    pub fn dimensions(&self) -> (usize, usize) {
        self.metadata.dimensions
    }

    /// Calculate basic image statistics: mean, median, min, max, and standard deviation
    pub fn calculate_statistics(&self) -> ImageStatistics {
        let mut min = f32::MAX;
        let mut max = f32::MIN;
        let mut sum = 0.0;

        // Calculate min, max, and sum
        for &value in self.data.iter() {
            sum += value;
            if value < min {
                min = value;
            }
            if value > max {
                max = value;
            }
        }

        let count = self.data.len() as f32;
        let mean = sum / count;

        // Calculate variance and standard deviation
        let mut variance_sum = 0.0;
        for &value in self.data.iter() {
            variance_sum += (value - mean).powi(2);
        }

        let std_dev = (variance_sum / count).sqrt();

        // Calculate median
        let mut values: Vec<f32> = self.data.iter().cloned().collect();
        values.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

        let median = if values.is_empty() {
            0.0
        } else if values.len() % 2 == 0 {
            let mid = values.len() / 2;
            (values[mid - 1] + values[mid]) / 2.0
        } else {
            values[values.len() / 2]
        };

        ImageStatistics {
            min,
            max,
            mean,
            median,
            std_dev,
        }
    }
}
