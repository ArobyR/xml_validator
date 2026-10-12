mod classification;
// Structured-diagnostics model for a future pass; not used by the UI yet.
#[allow(dead_code)]
mod diagnostics;
mod i18n;
mod validation;

use classification::{Detail, Phase, ValidationResult};
use eframe::egui;
use i18n::I18n;
use rfd::FileDialog;
use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::time::Duration;

const VALIDATION_TIMEOUT: Duration = Duration::from_secs(120);

const SUCCESS_COLOR: egui::Color32 = egui::Color32::from_rgb(22, 163, 74);
const ERROR_COLOR: egui::Color32 = egui::Color32::from_rgb(220, 38, 38);
const WARNING_COLOR: egui::Color32 = egui::Color32::from_rgb(217, 119, 6);
const PRIMARY_COLOR: egui::Color32 = egui::Color32::from_rgb(37, 99, 235);

#[derive(PartialEq, Eq, Clone, Copy)]
enum State {
    Idle,
    Validating,
}

#[derive(Clone, Copy)]
enum FileKind {
    Xml,
    Xsd,
}

struct ValidatorApp {
    i18n: I18n,
    xml_path: Option<PathBuf>,
    xsd_path: Option<PathBuf>,
    xmllint: Option<PathBuf>,
    state: State,
    receiver: Option<Receiver<ValidationResult>>,
    result: Option<ValidationResult>,
}

impl ValidatorApp {
    fn new() -> Self {
        Self {
            i18n: I18n::default(),
            xml_path: None,
            xsd_path: None,
            xmllint: validation::detect_xmllint(),
            state: State::Idle,
            receiver: None,
            result: None,
        }
    }

    fn start_validation(&mut self) {
        if self.xmllint.is_none() {
            self.result = Some(ValidationResult {
                phase: Phase::Environment,
                success: false,
                stderr: String::new(),
                exit_code: None,
                duration: Duration::ZERO,
                detail: Some(Detail::MissingXmllint),
            });
            return;
        }
        let Some(xml) = self.xml_path.clone() else {
            self.result = Some(ValidationResult {
                phase: Phase::Environment,
                success: false,
                stderr: String::new(),
                exit_code: None,
                duration: Duration::ZERO,
                detail: Some(Detail::MissingXml),
            });
            return;
        };
        let Some(xsd) = self.xsd_path.clone() else {
            self.result = Some(ValidationResult {
                phase: Phase::Environment,
                success: false,
                stderr: String::new(),
                exit_code: None,
                duration: Duration::ZERO,
                detail: Some(Detail::MissingXsd),
            });
            return;
        };
        let xmllint = self.xmllint.clone().expect("checked above");

        let (tx, rx) = mpsc::channel();
        self.receiver = Some(rx);
        self.state = State::Validating;
        self.result = None;

        std::thread::spawn(move || {
            let result = validation::run_and_classify(&xmllint, &xsd, &xml, VALIDATION_TIMEOUT);
            let _ = tx.send(result);
        });
    }

    fn process_messages(&mut self, ctx: &egui::Context) {
        let Some(rx) = &self.receiver else {
            return;
        };
        match rx.try_recv() {
            Ok(result) => {
                self.result = Some(result);
                self.state = State::Idle;
                self.receiver = None;
            }
            Err(TryRecvError::Empty) => {
                ctx.request_repaint_after(Duration::from_millis(120));
            }
            Err(TryRecvError::Disconnected) => {
                self.state = State::Idle;
                self.receiver = None;
                self.result = Some(ValidationResult {
                    phase: Phase::Environment,
                    success: false,
                    stderr: String::new(),
                    exit_code: None,
                    duration: Duration::ZERO,
                    detail: Some(Detail::ThreadLost),
                });
            }
        }
    }

    fn detail_text(&self, detail: &Detail) -> String {
        match detail {
            Detail::Timeout { seconds } => self
                .i18n
                .translate_args("detail.timeout", &[("s", &seconds.to_string())]),
            Detail::Spawn { error } => self
                .i18n
                .translate_args("detail.spawn", &[("error", error)]),
            Detail::MissingXml => self.i18n.translate("error.select_xml"),
            Detail::MissingXsd => self.i18n.translate("error.select_xsd"),
            Detail::MissingXmllint => self.i18n.translate("error.xmllint_missing"),
            Detail::ThreadLost => self.i18n.translate("detail.thread_lost"),
        }
    }

    fn accent_color(result: &ValidationResult) -> egui::Color32 {
        if result.success {
            return SUCCESS_COLOR;
        }
        match result.phase {
            Phase::SchemaViolation | Phase::MalformedXml => ERROR_COLOR,
            Phase::SchemaCompileError | Phase::SchemaNotFound | Phase::Environment => WARNING_COLOR,
        }
    }

    fn file_row(&mut self, ui: &mut egui::Ui, kind: FileKind) {
        let (title, path) = match kind {
            FileKind::Xml => (self.i18n.translate("file.xml"), self.xml_path.clone()),
            FileKind::Xsd => (self.i18n.translate("file.xsd"), self.xsd_path.clone()),
        };

        ui.horizontal(|ui| {
            ui.add_sized(
                [120.0, 22.0],
                egui::Label::new(egui::RichText::new(title).strong()),
            );

            let text = match &path {
                Some(p) => p.display().to_string(),
                None => self.i18n.translate("file.none"),
            };
            let width = (ui.available_width() - 110.0).max(120.0);
            let response = ui.add_sized(
                [width, 22.0],
                egui::Label::new(egui::RichText::new(text).monospace()).truncate(),
            );
            if let Some(p) = &path {
                response.on_hover_text(p.display().to_string());
            }

            if ui.button(self.i18n.translate("file.browse")).clicked() {
                let dialog = match kind {
                    FileKind::Xml => FileDialog::new().add_filter("XML", &["xml"]),
                    FileKind::Xsd => FileDialog::new().add_filter("XSD", &["xsd"]),
                };
                if let Some(p) = dialog.pick_file() {
                    match kind {
                        FileKind::Xml => self.xml_path = Some(p),
                        FileKind::Xsd => self.xsd_path = Some(p),
                    }
                }
            }
        });
    }

    fn environment_warning(&mut self, ui: &mut egui::Ui) {
        let color = WARNING_COLOR;
        egui::Frame::NONE
            .fill(egui::Color32::from_rgba_unmultiplied(
                color.r(),
                color.g(),
                color.b(),
                22,
            ))
            .stroke(egui::Stroke::new(
                1.0,
                egui::Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), 110),
            ))
            .corner_radius(egui::CornerRadius::same(8))
            .inner_margin(egui::Margin::same(14))
            .show(ui, |ui| {
                ui.label(
                    egui::RichText::new(self.i18n.translate("env.title"))
                        .strong()
                        .size(15.0)
                        .color(color),
                );
                ui.add_space(4.0);
                ui.label(self.i18n.translate("env.explanation"));
                ui.add_space(8.0);
                for key in [
                    "env.macos",
                    "env.macos_note",
                    "env.debian",
                    "env.fedora",
                    "env.windows",
                ] {
                    ui.label(
                        egui::RichText::new(self.i18n.translate(key))
                            .monospace()
                            .size(12.0),
                    );
                }
                ui.add_space(10.0);
                ui.horizontal(|ui| {
                    if ui.button(self.i18n.translate("button.redetect")).clicked() {
                        self.xmllint = validation::detect_xmllint();
                    }
                    if ui
                        .button(self.i18n.translate("button.locate_xmllint"))
                        .clicked()
                    {
                        if let Some(p) = FileDialog::new().pick_file() {
                            self.xmllint = Some(p);
                        }
                    }
                });
            });
    }

    fn result_panel(&self, ui: &mut egui::Ui, result: &ValidationResult) {
        let color = Self::accent_color(result);
        egui::Frame::NONE
            .fill(egui::Color32::from_rgba_unmultiplied(
                color.r(),
                color.g(),
                color.b(),
                22,
            ))
            .stroke(egui::Stroke::new(
                1.0,
                egui::Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), 110),
            ))
            .corner_radius(egui::CornerRadius::same(8))
            .inner_margin(egui::Margin::same(14))
            .show(ui, |ui| {
                let title = if result.success {
                    self.i18n.translate("result.success")
                } else {
                    self.i18n.translate(result.phase.title_key())
                };
                ui.label(egui::RichText::new(title).strong().size(16.0).color(color));
                ui.add_space(4.0);

                let explanation = if result.success {
                    self.i18n.translate("result.success_detail")
                } else {
                    self.i18n.translate(result.phase.explanation_key())
                };
                ui.label(explanation);

                if let Some(detail) = &result.detail {
                    ui.add_space(4.0);
                    ui.label(self.detail_text(detail));
                }

                ui.add_space(8.0);
                ui.horizontal_wrapped(|ui| {
                    if result.success {
                        ui.label(
                            egui::RichText::new(self.i18n.translate_args(
                                "report.no_errors",
                                &[("s", &format!("{:.2}", result.duration.as_secs_f32()))],
                            ))
                            .weak(),
                        );
                    } else {
                        ui.label(
                            egui::RichText::new(self.i18n.translate_args(
                                "report.errors",
                                &[("n", &result.error_count().to_string())],
                            ))
                            .weak(),
                        );
                        ui.separator();
                        ui.label(
                            egui::RichText::new(self.i18n.translate_args(
                                "report.duration",
                                &[("s", &format!("{:.2}", result.duration.as_secs_f32()))],
                            ))
                            .weak(),
                        );
                        if let Some(code) = result.exit_code {
                            ui.separator();
                            ui.label(
                                egui::RichText::new(self.i18n.translate_args(
                                    "report.exit_code",
                                    &[("code", &code.to_string())],
                                ))
                                .weak(),
                            );
                        }
                    }
                });
            });

        if !result.stderr.trim().is_empty() {
            ui.add_space(10.0);
            egui::CollapsingHeader::new(self.i18n.translate("detail.technical"))
                .id_salt("technical_details")
                .show(ui, |ui| {
                    ui.add(
                        egui::Label::new(
                            egui::RichText::new(&result.stderr).monospace().size(12.0),
                        )
                        .wrap(),
                    );
                });
        }
    }

    fn central_panel(&mut self, ui: &mut egui::Ui) {
        if self.xmllint.is_none() {
            self.environment_warning(ui);
            ui.add_space(14.0);
        }

        egui::Frame::group(ui.style()).show(ui, |ui| {
            self.file_row(ui, FileKind::Xml);
            ui.add_space(6.0);
            self.file_row(ui, FileKind::Xsd);
        });

        ui.add_space(18.0);

        ui.vertical_centered(|ui| {
            let label = if self.state == State::Validating {
                self.i18n.translate("button.validating")
            } else {
                self.i18n.translate("button.validate")
            };
            let button = egui::Button::new(
                egui::RichText::new(label)
                    .strong()
                    .size(15.0)
                    .color(egui::Color32::WHITE),
            )
            .min_size(egui::vec2(260.0, 48.0))
            .fill(PRIMARY_COLOR)
            .corner_radius(egui::CornerRadius::same(8));
            if ui.add_enabled(self.state == State::Idle, button).clicked() {
                self.start_validation();
            }
        });

        ui.add_space(18.0);
        ui.separator();
        ui.add_space(10.0);

        if self.state == State::Validating {
            ui.horizontal(|ui| {
                ui.spinner();
                ui.label(self.i18n.translate("status.validating"));
            });
        } else if let Some(result) = &self.result {
            self.result_panel(ui, result);
        } else {
            ui.label(egui::RichText::new(self.i18n.translate("status.idle")).weak());
        }
    }
}

impl eframe::App for ValidatorApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.process_messages(ctx);

        egui::TopBottomPanel::top("header").show(ctx, |ui| {
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                ui.heading(self.i18n.translate("app.title"));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    egui::ComboBox::from_id_salt("language_selector")
                        .selected_text(self.i18n.name())
                        .show_ui(ui, |ui| {
                            let names = self.i18n.names();
                            for (index, name) in names.iter().enumerate() {
                                if ui
                                    .selectable_label(self.i18n.index() == index, name)
                                    .clicked()
                                {
                                    self.i18n.select(index);
                                }
                            }
                        });
                    ui.label(self.i18n.translate("language"));
                });
            });
            ui.add_space(2.0);
            ui.label(egui::RichText::new(self.i18n.translate("app.subtitle")).weak());
            ui.add_space(6.0);
        });

        egui::TopBottomPanel::bottom("footer").show(ctx, |ui| {
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                match &self.xmllint {
                    Some(p) => ui.label(
                        egui::RichText::new(self.i18n.translate_args(
                            "footer.xmllint",
                            &[("path", &p.display().to_string())],
                        ))
                        .weak()
                        .small(),
                    ),
                    None => ui.label(
                        egui::RichText::new(self.i18n.translate("footer.xmllint_none"))
                            .weak()
                            .small()
                            .color(WARNING_COLOR),
                    ),
                };
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui
                        .small_button(self.i18n.translate("button.redetect"))
                        .clicked()
                    {
                        self.xmllint = validation::detect_xmllint();
                    }
                    if ui
                        .small_button(self.i18n.translate("button.locate_xmllint"))
                        .clicked()
                    {
                        if let Some(p) = FileDialog::new().pick_file() {
                            self.xmllint = Some(p);
                        }
                    }
                });
            });
            ui.add_space(4.0);
        });

        egui::CentralPanel::default().show(ctx, |ui| {
            self.central_panel(ui);
        });
    }
}

fn main() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([780.0, 640.0]),
        ..Default::default()
    };

    eframe::run_native(
        "XML Schema Validator",
        options,
        Box::new(|_cc| Ok(Box::new(ValidatorApp::new()))),
    )
}
