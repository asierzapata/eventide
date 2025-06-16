use eframe::egui;
use rfd::FileDialog;
use std::fs;
use std::path::PathBuf;

use crate::gui::registration::RegistrationView;
use crate::image::FrameType;

/// Represents a frame set that can contain:
/// - A directory path where the frames are located
/// - The list of loaded image file paths within that directory
/// - Optional: The actual images if/when loaded
pub struct FrameSet {
    pub frame_type: FrameType,
    pub directory: Option<PathBuf>,
    pub file_paths: Vec<PathBuf>,
    pub is_required: bool,
}

impl FrameSet {
    fn new(frame_type: FrameType, is_required: bool) -> Self {
        Self {
            frame_type,
            directory: None,
            file_paths: Vec::new(),
            is_required,
        }
    }

    fn frame_type_name(&self) -> &str {
        match self.frame_type {
            FrameType::Light => "Light",
            FrameType::Dark => "Dark",
            FrameType::Flat => "Flat",
            FrameType::Bias => "Bias",
            FrameType::DarkFlat => "Dark Flat",
        }
    }

    fn scan_directory(&mut self) {
        if let Some(dir) = &self.directory {
            match fs::read_dir(dir) {
                Ok(entries) => {
                    self.file_paths.clear();
                    for entry in entries.flatten() {
                        let path = entry.path();
                        if path.is_file() {
                            let extension =
                                path.extension().and_then(|ext| ext.to_str()).unwrap_or("");
                            // Filter for common astrophotography image formats
                            if ["fit", "fits", "fts"].contains(&extension.to_lowercase().as_str()) {
                                self.file_paths.push(path);
                            }
                        }
                    }
                    // Sort the files by name
                    self.file_paths.sort();
                }
                Err(e) => {
                    eprintln!("Error reading directory {}: {}", dir.display(), e);
                }
            }
        }
    }
}

/// Represents the current step in the processing workflow
#[derive(PartialEq)]
enum WorkflowStep {
    FolderSelection,
    Registration,
    Processing,
    Results,
}

pub struct EventideApp {
    frame_sets: Vec<FrameSet>,
    output_directory: Option<PathBuf>,
    // Workflow steps
    current_step: WorkflowStep,
    // Registration view
    registration_view: RegistrationView,
}

impl Default for EventideApp {
    fn default() -> Self {
        Self {
            frame_sets: vec![
                FrameSet::new(FrameType::Light, true),
                FrameSet::new(FrameType::Dark, false),
                FrameSet::new(FrameType::Flat, false),
                FrameSet::new(FrameType::Bias, false),
                FrameSet::new(FrameType::DarkFlat, false),
            ],
            output_directory: None,
            current_step: WorkflowStep::FolderSelection,
            registration_view: RegistrationView::new(),
        }
    }
}

impl EventideApp {
    pub fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        Self::default()
    }

    fn select_directory(&self) -> Option<PathBuf> {
        FileDialog::new()
            .set_title("Select directory")
            .pick_folder()
    }

    fn ui_frame_set(&mut self, ui: &mut egui::Ui, index: usize) {
        // Create a collapsible header for each frame type
        let is_required = self.frame_sets[index].is_required;
        let frame_type_name = self.frame_sets[index].frame_type_name().to_string();
        let has_directory = self.frame_sets[index].directory.is_some();
        let dir_display = self.frame_sets[index]
            .directory
            .as_ref()
            .map(|d| d.display().to_string());

        let required_text = if is_required {
            " (Required)"
        } else {
            " (Optional)"
        };
        let header_text = format!("{} Frames{}", frame_type_name, required_text);

        egui::CollapsingHeader::new(header_text)
            .default_open(is_required)
            .show(ui, |ui| {
                // Directory selection
                ui.horizontal(|ui| {
                    if let Some(dir_str) = dir_display {
                        ui.label("Directory:");
                        ui.monospace(dir_str);
                    } else {
                        ui.label("No directory selected");
                    }

                    if ui.button("Select Directory").clicked() {
                        if let Some(path) = self.select_directory() {
                            // Store index and path for later use
                            let frame_set = &mut self.frame_sets[index];
                            frame_set.directory = Some(path);
                            frame_set.scan_directory();
                        }
                    }

                    if has_directory && ui.button("Refresh").clicked() {
                        let frame_set = &mut self.frame_sets[index];
                        frame_set.scan_directory();
                    }

                    if has_directory && ui.button("Clear").clicked() {
                        let frame_set = &mut self.frame_sets[index];
                        frame_set.directory = None;
                        frame_set.file_paths.clear();
                    }
                });

                // Display file table if directory is selected
                let file_paths_clone = self.frame_sets[index].file_paths.clone();
                if !file_paths_clone.is_empty() {
                    ui.add_space(8.0);

                    egui::ScrollArea::vertical()
                        .max_height(200.0)
                        .show(ui, |ui| {
                            egui::Grid::new(format!("{}_files_grid", frame_type_name))
                                .striped(true)
                                .min_col_width(100.0)
                                .show(ui, |ui| {
                                    // Header row
                                    ui.strong("File Name");
                                    ui.end_row();

                                    // File rows
                                    for path in &file_paths_clone {
                                        if let Some(file_name) =
                                            path.file_name().and_then(|f| f.to_str())
                                        {
                                            ui.label(file_name);
                                            ui.end_row();
                                        }
                                    }
                                });
                        });

                    ui.label(format!("Total files: {}", file_paths_clone.len()));
                } else if has_directory {
                    ui.label("No compatible files found in the selected directory");
                }
            });
    }
}

impl EventideApp {
    // Move frames from folder selection to registration view
    fn load_frames_for_registration(&mut self) {
        for frame_set in &self.frame_sets {
            if !frame_set.file_paths.is_empty() {
                // Clone paths to ensure they're accessible for loading
                let paths: Vec<PathBuf> = frame_set.file_paths.clone();
                // Load frames into registration view
                self.registration_view
                    .load_frames_from_paths(frame_set.frame_type, paths);
            }
        }
    }

    fn render_workflow_navbar(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.selectable_value(
                &mut self.current_step,
                WorkflowStep::FolderSelection,
                "1. Folder Selection",
            );
            ui.selectable_value(
                &mut self.current_step,
                WorkflowStep::Registration,
                "2. Registration",
            );
            ui.selectable_value(
                &mut self.current_step,
                WorkflowStep::Processing,
                "3. Processing",
            );
            ui.selectable_value(&mut self.current_step, WorkflowStep::Results, "4. Results");
        });
        ui.separator();
    }

    fn render_folder_selection_step(&mut self, _ctx: &egui::Context, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.strong("Output directory:");

            if let Some(dir) = &self.output_directory {
                ui.monospace(dir.display().to_string());
            } else {
                ui.label("Not selected");
            }

            if ui.button("Select").clicked() {
                if let Some(path) = self.select_directory() {
                    self.output_directory = Some(path);
                }
            }

            if self.output_directory.is_some() && ui.button("Clear").clicked() {
                self.output_directory = None;
            }
        });

        ui.add_space(16.0);

        // Frame set sections
        for i in 0..self.frame_sets.len() {
            self.ui_frame_set(ui, i);
            ui.add_space(8.0);
        }

        ui.add_space(16.0);

        // Next step button
        let can_proceed = self.frame_sets[0].directory.is_some() && self.output_directory.is_some();

        ui.add_enabled_ui(can_proceed, |ui| {
            if ui.button("Continue to Registration").clicked() {
                self.load_frames_for_registration();
                self.current_step = WorkflowStep::Registration;
            }
        });

        if !can_proceed {
            ui.label("Select at least the Light frames directory and output directory to proceed");
        }
    }

    fn render_registration_step(&mut self, ctx: &egui::Context, ui: &mut egui::Ui) {
        // Display the registration view
        self.registration_view.ui(ctx, ui);

        ui.add_space(16.0);

        ui.horizontal(|ui| {
            if ui.button("< Back to Folder Selection").clicked() {
                self.current_step = WorkflowStep::FolderSelection;
            }

            // Only enable the Continue button if at least one light frame is selected
            let light_frames = self.registration_view.get_selected_frames(FrameType::Light);
            let can_continue = !light_frames.is_empty();

            ui.add_enabled_ui(can_continue, |ui| {
                if ui.button("Continue to Processing >").clicked() {
                    // TODO: Gather the selected frames for processing
                    self.current_step = WorkflowStep::Processing;
                }
            });

            if !can_continue {
                ui.label("Select at least one Light frame to continue");
            }
        });
    }

    fn render_processing_step(&mut self, _ctx: &egui::Context, ui: &mut egui::Ui) {
        ui.heading("Processing");
        ui.label("Processing options will be implemented here");

        ui.add_space(16.0);

        ui.horizontal(|ui| {
            if ui.button("< Back to Registration").clicked() {
                self.current_step = WorkflowStep::Registration;
            }

            if ui.button("Start Processing").clicked() {
                println!("Processing images...");
                // TODO: Implement actual processing
                self.current_step = WorkflowStep::Results;
            }
        });
    }

    fn render_results_step(&mut self, _ctx: &egui::Context, ui: &mut egui::Ui) {
        ui.heading("Results");
        ui.label("Results will be displayed here");

        ui.add_space(16.0);

        if ui.button("< Back to Processing").clicked() {
            self.current_step = WorkflowStep::Processing;
        }
    }
}

impl eframe::App for EventideApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("Eventide");

            ui.add_space(16.0);

            // Navigation between workflow steps
            self.render_workflow_navbar(ui);

            ui.add_space(8.0);

            // Render the current step
            match self.current_step {
                WorkflowStep::FolderSelection => self.render_folder_selection_step(ctx, ui),
                WorkflowStep::Registration => self.render_registration_step(ctx, ui),
                WorkflowStep::Processing => self.render_processing_step(ctx, ui),
                WorkflowStep::Results => self.render_results_step(ctx, ui),
            }
        });
    }
}
