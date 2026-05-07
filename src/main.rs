use eframe::egui;
use rfd::FileDialog;
use std::path::PathBuf;
use std::process::Command;

#[derive(Default)]
struct ValidadorApp {
    xml_path: Option<PathBuf>,
    xsd_path: Option<PathBuf>,
    resultado: String,
}

impl ValidadorApp {
    fn new() -> Self {
        Self {
            resultado: "👋 Selecciona un archivo XML y su esquema XSD para empezar.".to_owned(),
            ..Default::default()
        }
    }

    fn seleccionar_xml(&mut self) {
        if let Some(path) = FileDialog::new().add_filter("XML", &["xml"]).pick_file() {
            self.xml_path = Some(path);
        }
    }

    fn seleccionar_xsd(&mut self) {
        if let Some(path) = FileDialog::new().add_filter("XSD", &["xsd"]).pick_file() {
            self.xsd_path = Some(path);
        }
    }

    fn validar(&mut self) {
        let xml = match &self.xml_path {
            Some(p) => p,
            None => {
                self.resultado = "❌ Por favor selecciona un archivo XML.".to_string();
                return;
            }
        };
        let xsd = match &self.xsd_path {
            Some(p) => p,
            None => {
                self.resultado = "❌ Por favor selecciona un archivo XSD.".to_string();
                return;
            }
        };

        self.resultado = "⏳ Validando...".to_string();

        let output = Command::new("xmllint")
            .arg("--schema")
            .arg(xsd)
            .arg(xml)
            .arg("--noout")
            .output();

        match output {
            Ok(out) => {
                if out.status.success() {
                    self.resultado = "✅ ¡VALIDACIÓN EXITOSA!\n\n🎉 El documento XML cumple perfectamente con el esquema XSD.".to_string();
                } else {
                    let errores = String::from_utf8_lossy(&out.stderr);
                    self.resultado = format!("❌ EL XML NO CUMPLE CON EL ESQUEMA\n\nErrores encontrados:\n{}", errores);
                }
            }
            Err(e) => {
                self.resultado = format!(
                    "❌ No se encontró xmllint.\n\nError: {}\n\nInstala libxml2-utils / libxml2 como se indicó arriba.",
                    e
                );
            }
        }
    }
}

impl eframe::App for ValidadorApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.vertical_centered(|ui| {
                ui.heading("🔎 Validador XML contra XSD");
                ui.add_space(20.0);
            });

            // === XML ===
            ui.group(|ui| {
                ui.horizontal(|ui| {
                    if ui.button("📄 Seleccionar XML").clicked() {
                        self.seleccionar_xml();
                    }
                    if let Some(path) = &self.xml_path {
                        ui.monospace(path.file_name().unwrap().to_string_lossy());
                    } else {
                        ui.label("Ningún archivo XML seleccionado");
                    }
                });
            });

            ui.add_space(8.0);

            // === XSD ===
            ui.group(|ui| {
                ui.horizontal(|ui| {
                    if ui.button("📋 Seleccionar XSD").clicked() {
                        self.seleccionar_xsd();
                    }
                    if let Some(path) = &self.xsd_path {
                        ui.monospace(path.file_name().unwrap().to_string_lossy());
                    } else {
                        ui.label("Ningún archivo XSD seleccionado");
                    }
                });
            });

            ui.add_space(25.0);

            // Botón grande VALIDAR
            let btn = egui::Button::new("🚀 VALIDAR AHORA")
                .min_size(egui::vec2(250.0, 55.0))
                .fill(egui::Color32::from_rgb(39, 174, 96));
            if ui.add(btn).clicked() {
                self.validar();
            }

            ui.add_space(20.0);
            ui.separator();
            ui.add_space(10.0);

            ui.label("📋 Resultado de la validación:");
            egui::ScrollArea::vertical()
                .auto_shrink([false; 2])
                .max_height(320.0)
                .show(ui, |ui| {
                    ui.add(
                        egui::Label::new(
                            egui::RichText::new(&self.resultado)
                                .monospace()
                                .size(14.0)
                        )
                    );
                });
        });
    }
}

fn main() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([760.0, 620.0]),
        ..Default::default()
    };

    eframe::run_native(
        "Validador XML - XSD",
        options,
        Box::new(|_cc| Ok(Box::new(ValidadorApp::new()))),
    )
}