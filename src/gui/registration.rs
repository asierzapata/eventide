use eframe::egui::{self, ComboBox, Context, Grid, ScrollArea, Ui, Vec2};
use egui::Widget;
use std::path::PathBuf;

use crate::image::{FitsImage, FrameType, ImageError};

/// Represents different stretching methods to enhance image visualization
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StretchMethod {
    /// Linear stretch - simple min/max normalization
    Linear,
    /// Logarithmic stretch - enhances dim features
    Logarithmic,
    /// Auto stretch - automatic histogram adjustment
    AutoStretch,
}

impl Default for StretchMethod {
    fn default() -> Self {
        StretchMethod::Linear
    }
}

/// Represents a frame in the registration process
#[derive(Clone)]
pub struct RegisteredFrame {
    /// Path to the image file
    pub path: PathBuf,
    /// Metadata extracted from the image
    pub fits_image: FitsImage,
    /// Whether this frame is selected for processing
    pub selected: bool,
    /// Thumbnail or preview data (will be loaded on demand)
    pub preview_data: Option<egui::TextureHandle>,
    /// The stretch method used for the current preview
    pub preview_stretch: Option<StretchMethod>,
}

impl RegisteredFrame {
    pub fn new(path: PathBuf, frame_type: FrameType) -> Self {
        let fits_image =
            FitsImage::from_file(&path, frame_type).unwrap_or_else(|_| FitsImage::new(0, 0));
        Self {
            path,
            fits_image,
            selected: true, // Default to selected
            preview_data: None,
            preview_stretch: None, // No preview generated yet
        }
    }

    /// Generate a preview image for display
    pub fn generate_preview(
        &mut self,
        ctx: &Context,
        stretch_method: StretchMethod,
    ) -> Result<(), ImageError> {
        // If we already have a preview with the same stretch method, don't regenerate it
        // This improves performance when switching between tabs
        if self.preview_data.is_some() && self.preview_stretch == Some(stretch_method) {
            return Ok(());
        }

        // Scale image data to 8-bit for preview
        let data = self.fits_image.data.clone();
        let flat_data = data.iter().cloned().collect::<Vec<f32>>();

        // Find min and max for scaling
        let min_val = flat_data.iter().fold(f32::INFINITY, |a, &b| a.min(b));
        let max_val = flat_data.iter().fold(f32::NEG_INFINITY, |a, &b| a.max(b));
        let range = max_val - min_val;

        // Calculate statistics needed for stretching
        let mean = flat_data.iter().sum::<f32>() / flat_data.len() as f32;
        let std_dev = (flat_data.iter().map(|&x| (x - mean).powi(2)).sum::<f32>()
            / flat_data.len() as f32)
            .sqrt();

        // Create 8-bit RGB data for preview
        let width = self.fits_image.metadata.dimensions.0;
        let height = self.fits_image.metadata.dimensions.1;
        let mut rgba_data = Vec::with_capacity(width * height * 4);

        // Convert grayscale data to RGBA using the selected stretch method
        for value in flat_data {
            let normalized = if range > 0.0 {
                match stretch_method {
                    StretchMethod::Linear => {
                        // Simple linear stretch
                        ((value - min_val) / range * 255.0).clamp(0.0, 255.0) as u8
                    }
                    StretchMethod::Logarithmic => {
                        // Logarithmic stretch - enhances dim features
                        if value <= min_val {
                            0
                        } else {
                            let epsilon = 0.001; // To avoid ln(0)
                            ((value - min_val + epsilon).ln() / (max_val - min_val + epsilon).ln()
                                * 255.0)
                                .clamp(0.0, 255.0) as u8
                        }
                    }
                    StretchMethod::AutoStretch => {
                        // Automatic stretching based on mean and std dev
                        // Using a simple algorithm that enhances contrast around the mean
                        let shadow_clip = (mean - 2.0 * std_dev).max(min_val);
                        let highlight_clip = (mean + 4.0 * std_dev).min(max_val);
                        let auto_range = highlight_clip - shadow_clip;
                        if auto_range > 0.0 {
                            ((value - shadow_clip) / auto_range * 255.0).clamp(0.0, 255.0) as u8
                        } else {
                            0
                        }
                    }
                }
            } else {
                0
            };

            // Add RGB and alpha channels
            rgba_data.push(normalized);
            rgba_data.push(normalized);
            rgba_data.push(normalized);
            rgba_data.push(255); // Alpha
        }

        // Create egui texture
        let texture = ctx.load_texture(
            self.path.file_name().unwrap_or_default().to_string_lossy(),
            egui::ColorImage::from_rgba_unmultiplied([width, height], &rgba_data),
            egui::TextureOptions::default(),
        );

        self.preview_data = Some(texture);
        self.preview_stretch = Some(stretch_method);

        Ok(())
    }
}

/// The registration view state
pub struct RegistrationView {
    /// Currently selected tab
    pub active_tab: FrameType,
    /// Frames organized by frame type
    pub frames: std::collections::HashMap<FrameType, Vec<RegisteredFrame>>,
    /// Currently selected frame index for each tab
    pub selected_frame_indices: std::collections::HashMap<FrameType, Option<usize>>,
    /// Currently selected stretch method for image preview
    pub selected_stretch: StretchMethod,
}

impl Default for RegistrationView {
    fn default() -> Self {
        let mut selected_frame_indices = std::collections::HashMap::new();
        for frame_type in [
            FrameType::Light,
            FrameType::Dark,
            FrameType::Flat,
            FrameType::Bias,
            FrameType::DarkFlat,
        ]
        .iter()
        {
            selected_frame_indices.insert(*frame_type, None);
        }

        Self {
            active_tab: FrameType::Light,
            frames: std::collections::HashMap::new(),
            selected_frame_indices,
            selected_stretch: StretchMethod::default(),
        }
    }
}

impl RegistrationView {
    pub fn new() -> Self {
        Self::default()
    }

    /// Generate a preview for a frame with the specified stretching method
    fn regenerate_preview(
        &mut self,
        frame_type: FrameType,
        index: usize,
        ctx: &Context,
    ) -> Result<(), ImageError> {
        if let Some(frames) = self.frames.get_mut(&frame_type) {
            if index < frames.len() {
                // Remove existing preview to force regeneration with new stretch
                frames[index].preview_data = None;
                frames[index].preview_stretch = None;

                // Now ensure the preview is generated with the current stretch method
                return self.ensure_preview(frame_type, index, ctx);
            }
        }
        Ok(())
    }

    pub fn load_frames_from_paths(&mut self, frame_type: FrameType, paths: Vec<PathBuf>) {
        let mut frames = Vec::new();

        for path in paths {
            frames.push(RegisteredFrame::new(path, frame_type));
        }

        self.frames.insert(frame_type, frames);

        // Set the first frame as selected if there are frames
        if !self
            .frames
            .get(&frame_type)
            .unwrap_or(&Vec::new())
            .is_empty()
        {
            self.selected_frame_indices.insert(frame_type, Some(0));
        }
    }

    fn ensure_preview(
        &mut self,
        frame_type: FrameType,
        index: usize,
        ctx: &Context,
    ) -> Result<(), ImageError> {
        if let Some(frames) = self.frames.get_mut(&frame_type) {
            if index < frames.len() {
                // Generate the preview if needed or if stretch method changed
                let stretch = self.selected_stretch;
                if frames[index].preview_data.is_none()
                    || frames[index].preview_stretch != Some(stretch)
                {
                    println!(
                        "Generating preview for frame {} of type {:?} with {:?} stretch",
                        frames[index].path.display(),
                        frame_type,
                        stretch
                    );
                    return frames[index].generate_preview(ctx, stretch);
                }
            }
        }
        Ok(())
    }

    // TODO: Fix Image taking all the space and not letting the other elements render properly.
    // Only happens inside the horizontal divison of the registration view.
    fn render_frame_preview(&mut self, ui: &mut Ui, frame_type: FrameType) {
        if let Some(selected) = self
            .selected_frame_indices
            .get(&frame_type)
            .unwrap_or(&None)
        {
            if let Some(frames) = self.frames.get(&frame_type) {
                if *selected < frames.len() {
                    let frame = &frames[*selected];

                    // Add stretch method dropdown
                    ui.label("Stretch method:");
                    let current_stretch = self.selected_stretch;

                    ComboBox::from_id_source("stretch_method_combo")
                        .selected_text(match self.selected_stretch {
                            StretchMethod::Linear => "Linear",
                            StretchMethod::Logarithmic => "Logarithmic",
                            StretchMethod::AutoStretch => "AutoStretch",
                        })
                        .show_ui(ui, |ui| {
                            ui.selectable_value(
                                &mut self.selected_stretch,
                                StretchMethod::Linear,
                                "Linear",
                            );
                            ui.selectable_value(
                                &mut self.selected_stretch,
                                StretchMethod::Logarithmic,
                                "Logarithmic",
                            );
                            ui.selectable_value(
                                &mut self.selected_stretch,
                                StretchMethod::AutoStretch,
                                "AutoStretch",
                            );
                        });

                    // If preview data is available, display it
                    if let Some(texture) = &frame.preview_data {
                        // Calculate image size to fit the available space
                        let available_width = ui.available_width();
                        let available_height = ui.available_height() - 200.0; // Reserve space for metadata below

                        // Get image dimensions
                        let image_width = frame.fits_image.metadata.dimensions.0 as f32;
                        let image_height = frame.fits_image.metadata.dimensions.1 as f32;

                        // Calculate scale factor to fit in the available space
                        let scale_w = available_width / image_width;
                        let scale_h = available_height / image_height;
                        let scale = scale_w.min(scale_h);

                        // Calculate the displayed size
                        let display_width = image_width * scale;
                        let display_height = image_height * scale;

                        println!(
                            "Displaying preview for frame {}: {}x{} at scale {:.2}",
                            frame.path.display(),
                            display_width,
                            display_height,
                            scale
                        );

                        ui.centered_and_justified(|ui| {
                            ui.add(
                                egui::Image::new(texture)
                                    .fit_to_exact_size(Vec2::new(display_width, display_height))
                                    .corner_radius(4.0)
                                    .sense(egui::Sense::click()),
                            )
                        });
                    } else {
                        ui.label("Preview not available");
                    }

                    // Display some basic metadata
                    ui.label(format!(
                        "File: {}",
                        frame.path.file_name().unwrap_or_default().to_string_lossy()
                    ));
                    ui.label(format!(
                        "Dimensions: {}x{}",
                        frame.fits_image.metadata.dimensions.0,
                        frame.fits_image.metadata.dimensions.1
                    ));

                    if let Some(exposure) = frame.fits_image.metadata.exposure_time {
                        ui.label(format!("Exposure: {:.2} seconds", exposure));
                    }

                    if let Some(filter) = &frame.fits_image.metadata.filter {
                        ui.label(format!("Filter: {}", filter));
                    }

                    if let Some(gain) = frame.fits_image.metadata.iso_gain {
                        ui.label(format!("Gain: {}", gain));
                    }

                    if let Some(temp) = frame.fits_image.metadata.temperature {
                        ui.label(format!("Temperature: {:.1}°C", temp));
                    }

                    ui.label(format!(
                        "Pixel Type: {}",
                        match frame.fits_image.metadata.pixel_type {
                            crate::image::PixelType::F32 => "F32",
                            crate::image::PixelType::F64 => "F64",
                            crate::image::PixelType::U8 => "U8",
                            crate::image::PixelType::U16 => "U16",
                            crate::image::PixelType::U32 => "U32",
                            crate::image::PixelType::I16 => "I16",
                            crate::image::PixelType::I32 => "I32",
                        }
                    ));
                }
            }
        } else {
            ui.label("No image selected");
        }
    }

    fn render_frame_table(&mut self, ui: &mut Ui, frame_type: FrameType) {
        if let Some(frames) = self.frames.get_mut(&frame_type) {
            if frames.is_empty() {
                ui.label("No frames available");
                return;
            }

            ScrollArea::vertical()
                .id_salt(format!("table_scroll_{:?}", frame_type))
                .min_scrolled_height(600.0)
                .show(ui, |ui| {
                    Grid::new(format!("frames_table_{:?}", frame_type))
                        .num_columns(6)
                        .striped(true)
                        .min_col_width(60.0)
                        .show(ui, |ui| {
                            // Header row
                            ui.strong("Use");
                            ui.strong("Filename");
                            ui.strong("Exposure");
                            ui.strong("Filter");
                            ui.strong("Gain");
                            ui.strong("Temperature");
                            ui.strong("Preview");
                            ui.end_row();

                            // Data rows
                            for (idx, frame) in frames.iter_mut().enumerate() {
                                // Checkbox for selection
                                let mut selected = frame.selected;
                                if ui.checkbox(&mut selected, "").changed() {
                                    frame.selected = selected;
                                }

                                // File name
                                let file_name = frame
                                    .path
                                    .file_name()
                                    .unwrap_or_default()
                                    .to_string_lossy()
                                    .to_string();
                                ui.label(&file_name);

                                // Exposure time
                                if let Some(exposure) = frame.fits_image.metadata.exposure_time {
                                    ui.label(format!("{:.2}s", exposure));
                                } else {
                                    ui.label("-");
                                }

                                // Filter
                                if let Some(filter) = &frame.fits_image.metadata.filter {
                                    ui.label(filter);
                                } else {
                                    ui.label("-");
                                }

                                // Gain
                                if let Some(gain) = frame.fits_image.metadata.iso_gain {
                                    ui.label(format!("{}", gain));
                                } else {
                                    ui.label("-");
                                }

                                // Temperature
                                if let Some(temp) = frame.fits_image.metadata.temperature {
                                    ui.label(format!("{:.1}°C", temp));
                                } else {
                                    ui.label("-");
                                }

                                // Preview button with different styling for currently selected image
                                let is_selected = self.selected_frame_indices.get(&frame_type)
                                    == Some(&Some(idx));
                                let button_text = if is_selected { "Selected" } else { "View" };
                                if ui.button(button_text).clicked() {
                                    if let Some(selected_idx) =
                                        self.selected_frame_indices.get_mut(&frame_type)
                                    {
                                        *selected_idx = Some(idx);
                                    }
                                }

                                ui.end_row();
                            }
                        });
                });
        } else {
            ui.label("No frames loaded");
        }
    }

    /// Render the registration view UI
    pub fn ui(&mut self, ctx: &Context, ui: &mut Ui) {
        println!("Available width: {}", ui.available_width());
        println!("Available height: {}", ui.available_height());

        // Tab bar for different frame types
        ui.horizontal(|ui| {
            for frame_type in [
                FrameType::Light,
                FrameType::Dark,
                FrameType::Flat,
                FrameType::Bias,
                FrameType::DarkFlat,
            ]
            .iter()
            {
                let tab_name = match frame_type {
                    FrameType::Light => "Light",
                    FrameType::Dark => "Dark",
                    FrameType::Flat => "Flat",
                    FrameType::Bias => "Bias",
                    FrameType::DarkFlat => "Dark Flat",
                };

                // Count total vs selected frames
                let total_count;
                let selected_count;

                // Create a temporary vector to avoid borrowing issues
                let empty_vec = Vec::new();
                let frames = match self.frames.get(frame_type) {
                    Some(f) => f,
                    None => &empty_vec,
                };

                total_count = frames.len();
                selected_count = frames.iter().filter(|f| f.selected).count();

                let tab_text = format!("{} ({}/{})", tab_name, selected_count, total_count);

                if ui
                    .selectable_label(self.active_tab == *frame_type, tab_text)
                    .clicked()
                {
                    self.active_tab = *frame_type;
                }
            }
        });

        ui.separator();

        ui.add_space(8.0);

        // If there are frames for this type, ensure preview for the selected frame
        if let Some(selected) = self
            .selected_frame_indices
            .get(&self.active_tab)
            .unwrap_or(&None)
        {
            let _ = self.ensure_preview(self.active_tab, *selected, ctx);
        }

        println!(
            "Available height before horizontal: {}",
            ui.available_height()
        );

        let horizontal_ui_height = ui.available_height() - 100.0; // Reserve space for controls below

        // Use a horizontal layout with controlled sizing for preview and table
        ui.horizontal(|ui| {
            println!(
                "Available size horizontal: {}x{}, horiontal_ui_height: {}",
                ui.available_width(),
                ui.available_height(),
                horizontal_ui_height
            );
            ui.set_height(horizontal_ui_height);

            // Left side: Preview section with fixed width
            ui.vertical(|ui| {
                println!(
                    "Available size for preview: {}x{}",
                    ui.available_width(),
                    ui.available_height()
                );
                let half_available_width = ui.available_width() * 0.5;
                ui.set_width(half_available_width);
                ui.set_height(horizontal_ui_height);
                ui.group(|ui| {
                    ui.heading("Preview");
                    ui.set_width(half_available_width);
                    ui.set_height(ui.available_height());
                    println!(
                        "Preview section size: {}x{}",
                        ui.available_width(),
                        ui.available_height()
                    );
                    self.render_frame_preview(ui, self.active_tab);
                });
            });

            ui.add_space(8.0);

            // Right side: Table section that takes remaining space
            ui.vertical(|ui| {
                ui.group(|ui| {
                    ui.heading("Frame Selection");

                    ui.add_space(8.0);

                    self.render_frame_table(ui, self.active_tab);

                    ui.add_space(8.0);

                    // Add selection controls
                    ui.horizontal(|ui| {
                        if ui.button("Select All").clicked() {
                            if let Some(frames) = self.frames.get_mut(&self.active_tab) {
                                for frame in frames {
                                    frame.selected = true;
                                }
                            }
                        }
                        if ui.button("Deselect All").clicked() {
                            if let Some(frames) = self.frames.get_mut(&self.active_tab) {
                                for frame in frames {
                                    frame.selected = false;
                                }
                            }
                        }
                    });
                });
            });
        });
    }

    /// Get all selected frames of a specific type
    pub fn get_selected_frames(&self, frame_type: FrameType) -> Vec<PathBuf> {
        self.frames
            .get(&frame_type)
            .map(|frames| {
                frames
                    .iter()
                    .filter(|frame| frame.selected)
                    .map(|frame| frame.path.clone())
                    .collect()
            })
            .unwrap_or_default()
    }
}
