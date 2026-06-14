#![windows_subsystem = "windows"]

use eframe::egui;
use rfd::FileDialog;
use std::collections::HashSet;
use std::fs::{copy, create_dir_all, File};
use std::io::Write;
use std::path::PathBuf;
use std::process::Command;

use memmap::Mmap;
use rayon::prelude::*;
use walkdir::WalkDir;
use zip::write::FileOptions;

use num_traits::ToPrimitive;
use sims3_core::dbpf::filetypes::ResourceType;
use sims3_core::dbpf::DBPF;
use sims3_core::sims3pack_resolver::resolve_input;

// =============================
// CONFIGURAÇÕES DE LAYOUT
// =============================
const BREAKPOINT: f32 = 700.0;
const GAP: f32 = 60.0;
const MOBILE_MAX_WIDTH: f32 = 350.0;
const DESKTOP_TOTAL_WIDTH: f32 = 600.0;
const DESKTOP_TOTAL_HEIGHT: f32 = 420.0;

// Cores
const BUTTON_COLOR: egui::Color32 = egui::Color32::from_rgb(43, 163, 72);
const BUTTON_COLOR_SELECTED: egui::Color32 = egui::Color32::from_rgb(0, 132, 44);
const HOVER_COLOR: egui::Color32 = egui::Color32::from_rgb(27, 27, 27);
const TEXT_COLOR: egui::Color32 = egui::Color32::from_gray(200);

// =============================
// SUPORTE A IDIOMAS
// =============================
#[derive(Clone, Copy, PartialEq)]
enum Idioma {
    En,
    Es,
    Pt,
}

fn t(chave: &'static str, lang: Idioma) -> &'static str {
    match lang {
        Idioma::Pt => match chave {
            "title" => "Veiga's S3CC Splitter",
            "choose_file" => "Escolher Sim ou Lote",
            "choose_file_tip" => "Selecionar no formato .sims3pack ou .package",
            "mods_folder" => "Mods",
            "mods_folder_tip" => "Selecionar a pasta Mods do jogo",
            "dest_folder" => "Destino",
            "dest_folder_tip" => "Selecionar a pasta de saída para exportar os arquivos",
            "verify" => "Verificar CCs",
            "verify_tip" => "Verificar CCs compatíveis no pacote escolhido",
            "export" => "Exportar CCs",
            "export_tip" => "Exportar os CCs encontrados para a pasta destino",
            "zip" => "Compactar CCs",
            "zip_tip" => "Compactar os CCs encontrados em um arquivo ZIP",
            "verified" => "✔ CCs Verificados",
            "none" => "Nenhum CC verificado ainda.",
            "found" => "encontrados:",
            "lang_label" => "Idioma:",
            "pt_btn" => "PT",
            "es_btn" => "ES",
            "en_btn" => "EN",
            _ => chave,
        },
        Idioma::Es => match chave {
            "title" => "Veiga's S3CC Splitter",
            "choose_file" => "Elegir Sim o Solar",
            "choose_file_tip" => "Seleccionar en formato .sims3pack o .package",
            "mods_folder" => "Mods",
            "mods_folder_tip" => "Seleccionar la carpeta Mods del juego",
            "dest_folder" => "Destino",
            "dest_folder_tip" => "Seleccionar la carpeta de salida",
            "verify" => "Verificar CCs",
            "verify_tip" => "Verificar CCs compatibles en el paquete elegido",
            "export" => "Exportar CCs",
            "export_tip" => "Exportar los CCs encontrados a la carpeta destino",
            "zip" => "Comprimir CCs",
            "zip_tip" => "Comprimir los CCs encontrados en un archivo ZIP",
            "verified" => "✔ CCs Verificados",
            "none" => "Ningún CC verificado aún.",
            "found" => "encontrados:",
            "lang_label" => "Idioma:",
            "pt_btn" => "PT",
            "es_btn" => "ES",
            "en_btn" => "EN",
            _ => chave,
        },
        Idioma::En => match chave {
            "title" => "Veiga's S3CC Splitter",
            "choose_file" => "Choose Sim or Lot",
            "choose_file_tip" => "Select .sims3pack or .package format",
            "mods_folder" => "Mods",
            "mods_folder_tip" => "Select the game Mods folder",
            "dest_folder" => "Destination",
            "dest_folder_tip" => "Select the output folder",
            "verify" => "Verify CCs",
            "verify_tip" => "Check compatible CCs in the chosen package",
            "export" => "Export CCs",
            "export_tip" => "Export the found CCs to the destination folder",
            "zip" => "Compress CCs",
            "zip_tip" => "Compress the found CCs into a ZIP file",
            "verified" => "✔ Verified CCs",
            "none" => "No CC verified yet.",
            "found" => "found:",
            "lang_label" => "Language:",
            "pt_btn" => "PT",
            "es_btn" => "ES",
            "en_btn" => "EN",
            _ => chave,
        },
    }
}

// =============================
// ESTADO DA APLICAÇÃO
// =============================
struct LauncherApp {
    lang: Idioma,
    arquivo_x: Option<PathBuf>,
    pasta_origem: Option<PathBuf>,
    pasta_destino: Option<PathBuf>,
    arquivos_listados: Vec<PathBuf>,
    check_done: bool,
    copiar_clicked: bool,
    compactar_clicked: bool,
}

impl Default for LauncherApp {
    fn default() -> Self {
        Self {
            lang: Idioma::En,
            arquivo_x: None,
            pasta_origem: None,
            pasta_destino: None,
            arquivos_listados: vec![],
            check_done: false,
            copiar_clicked: false,
            compactar_clicked: false,
        }
    }
}

impl LauncherApp {
    fn filter_tgi_into_map(package: &DBPF<'_>, _merged: bool) -> HashSet<(u32, u32, u64)> {
        HashSet::from_iter(
            package
                .files
                .iter()
                .filter(|entry| {
                    entry.resource_type == ResourceType::CASP.to_u32().unwrap()
                        || entry.resource_type == ResourceType::FACE.to_u32().unwrap()
                        || entry.resource_type == ResourceType::SkinTone.to_u32().unwrap()
                        || entry.resource_type == ResourceType::XMLResource.to_u32().unwrap()
                })
                .map(|entry| (entry.resource_type, entry.resource_group, entry.instance)),
        )
    }

    fn processar(&mut self) {
        // limpa resultados anteriores e reseta flags
        self.arquivos_listados.clear();
        self.check_done = false;
        self.copiar_clicked = false;
        self.compactar_clicked = false;

        if let (Some(input_path), Some(origem)) = (&self.arquivo_x, &self.pasta_origem) {
            // Se for .sims3pack, converte automaticamente para um .package mergeado em pasta temporária.
            let resolved = resolve_input(input_path)
                .expect("Erro processando entrada (.sims3pack ou .package)");
            let merged_path = &resolved.merged_package;

            let mem = File::open(merged_path)
                .and_then(|f| unsafe { Mmap::map(&f) })
                .expect("Erro abrindo arquivo X");

            let merged = DBPF::new(&mem).expect("Arquivo X inválido");
            let find = Self::filter_tgi_into_map(&merged, true);

            let mut unicos: HashSet<String> = HashSet::new();
            let mut resultados: Vec<PathBuf> = Vec::new();

            for entry in WalkDir::new(origem).into_iter().filter_map(|e| e.ok()) {
                if entry
                    .path()
                    .extension()
                    .map(|ext| ext == "package")
                    .unwrap_or(false)
                {
                    if let Ok(mem) = File::open(entry.path()).and_then(|f| unsafe { Mmap::map(&f) })
                    {
                        if let Ok(package) = DBPF::new(&mem) {
                            let this_map = Self::filter_tgi_into_map(&package, false);
                            if !this_map.is_disjoint(&find) {
                                if let Some(name) = entry
                                    .path()
                                    .file_name()
                                    .map(|n| n.to_string_lossy().to_string())
                                {
                                    if unicos.insert(name.clone()) {
                                        resultados.push(entry.path().to_path_buf());
                                    }
                                }
                            }
                        }
                    }
                }
            }

            resultados.sort_by(|a, b| {
                a.file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .to_lowercase()
                    .cmp(
                        &b.file_name()
                            .unwrap_or_default()
                            .to_string_lossy()
                            .to_lowercase(),
                    )
            });

            self.arquivos_listados = resultados;
            self.check_done = true;
        }
    }

    fn exportar(&self) {
        if let (Some(origem), Some(destino), Some(arquivo)) =
            (&self.pasta_origem, &self.pasta_destino, &self.arquivo_x)
        {
            let base_name = arquivo
                .file_stem()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string();

            let final_dest = destino.join(base_name);
            let _ = create_dir_all(&final_dest);

            for f in &self.arquivos_listados {
                if let Ok(rel_path) = f.strip_prefix(origem) {
                    let dst_path = final_dest.join(rel_path);
                    let _ = create_dir_all(dst_path.parent().unwrap_or(&final_dest));
                    let _ = copy(f, &dst_path);
                } else if let Some(filename) = f.file_name() {
                    let dst = final_dest.join(filename);
                    let _ = copy(f, &dst);
                }
            }

            #[cfg(target_os = "windows")]
            {
                let _ = Command::new("explorer").arg(destino).spawn();
            }
            #[cfg(target_os = "macos")]
            {
                let _ = Command::new("open").arg(destino).spawn();
            }
            #[cfg(target_os = "linux")]
            {
                let _ = Command::new("xdg-open").arg(destino).spawn();
            }
        }
    }

    fn compactar(&self) {
        if let (Some(origem), Some(destino), Some(arquivo)) =
            (&self.pasta_origem, &self.pasta_destino, &self.arquivo_x)
        {
            let base_name = arquivo
                .file_stem()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string();

            let zip_path = destino.join(format!("{}.zip", base_name));
            let file = File::create(&zip_path).expect("Erro criando ZIP");
            let mut zip = zip::ZipWriter::new(file);

            let options =
                FileOptions::default().compression_method(zip::CompressionMethod::Deflated);

            for f in &self.arquivos_listados {
                let caminho_no_zip = if let Ok(rel_path) = f.strip_prefix(origem) {
                    // Mantém a mesma estrutura da pasta exportada:
                    // NomeDoArquivo/subpasta/arquivo.package
                    PathBuf::from(&base_name).join(rel_path)
                } else if let Some(filename) = f.file_name() {
                    PathBuf::from(&base_name).join(filename)
                } else {
                    continue;
                };

                // ZIP usa / como separador interno
                let caminho_no_zip = caminho_no_zip.to_string_lossy().replace('\\', "/");

                let data = std::fs::read(f).expect("Erro lendo arquivo");
                zip.start_file(caminho_no_zip, options)
                    .expect("Erro start_file ZIP");
                zip.write_all(&data).expect("Erro escrevendo no ZIP");
            }

            zip.finish().expect("Erro finalizando ZIP");

            #[cfg(target_os = "windows")]
            {
                let _ = Command::new("explorer").arg(destino).spawn();
            }
            #[cfg(target_os = "macos")]
            {
                let _ = Command::new("open").arg(destino).spawn();
            }
            #[cfg(target_os = "linux")]
            {
                let _ = Command::new("xdg-open").arg(destino).spawn();
            }
        }
    }

    fn render_select_button(
        ui: &mut egui::Ui,
        label: &str,
        size: [f32; 2],
        active: bool,
    ) -> egui::Response {
        let color = if active {
            BUTTON_COLOR_SELECTED
        } else {
            BUTTON_COLOR
        };

        ui.add_sized(
            size,
            egui::Button::new(egui::RichText::new(label).color(egui::Color32::WHITE))
                .fill(color)
                .rounding(10.0),
        )
    }

    fn render_language_buttons(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new(t("lang_label", self.lang)).color(TEXT_COLOR));

            if Self::render_lang_button(
                ui,
                t("en_btn", self.lang),
                [30.0, 15.0],
                self.lang == Idioma::En,
            )
            .clicked()
            {
                self.lang = Idioma::En;
            }
            if Self::render_lang_button(
                ui,
                t("es_btn", self.lang),
                [30.0, 15.0],
                self.lang == Idioma::Es,
            )
            .clicked()
            {
                self.lang = Idioma::Es;
            }
            if Self::render_lang_button(
                ui,
                t("pt_btn", self.lang),
                [30.0, 15.0],
                self.lang == Idioma::Pt,
            )
            .clicked()
            {
                self.lang = Idioma::Pt;
            }
        });
        ui.add_space(10.0);
    }

    fn render_lang_button(
        ui: &mut egui::Ui,
        label: &str,
        size: [f32; 2],
        active: bool,
    ) -> egui::Response {
        let color = if active {
            BUTTON_COLOR_SELECTED
        } else {
            BUTTON_COLOR
        };

        ui.add_sized(
            size,
            egui::Button::new(egui::RichText::new(label).color(egui::Color32::WHITE))
                .fill(color)
                .rounding(4.0), // 🔹 borda menor
        )
    }
}

// =============================
// RENDER INTERFACE
// =============================
impl eframe::App for LauncherApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        ctx.set_visuals(egui::Visuals {
            window_fill: HOVER_COLOR,
            ..Default::default()
        });

        // =============================
        // BOTÕES DE IDIOMA NO TOPO ESQUERDO
        // =============================
        egui::TopBottomPanel::top("top_bar").show(ctx, |ui| {
            ui.horizontal_wrapped(|ui| {
                self.render_language_buttons(ui);
            });
        });

        // =============================
        // RESTO DO LAYOUT
        // =============================
        egui::CentralPanel::default().show(ctx, |ui| {
            let available_size = ui.available_size();
            let desired_width = if available_size.x < BREAKPOINT {
                MOBILE_MAX_WIDTH
            } else {
                DESKTOP_TOTAL_WIDTH
            };
            let desired_height = DESKTOP_TOTAL_HEIGHT;

            let mut rect = egui::Rect::from_center_size(
                ui.max_rect().center(),
                egui::vec2(desired_width, desired_height),
            );

            rect = rect.translate(egui::vec2(0.0, -60.0));

            ui.allocate_ui_at_rect(rect, |ui| {
                if ui.available_width() < BREAKPOINT {
                    ui.vertical_centered(|ui| {
                        ui.set_max_width(MOBILE_MAX_WIDTH);
                        self.render_left(ui);
                        ui.add_space(30.0);
                        self.render_right(ui);
                    });
                } else {
                    ui.columns(5, |cols| {
                        cols[1].set_min_width(220.0);
                        self.render_left(&mut cols[1]);
                        cols[2].set_min_width(GAP);
                        cols[3].set_min_width(300.0);
                        self.render_right(&mut cols[3]);
                    });
                }
            });
        });
    }
}

// =============================
// Coluna da esquerda
// =============================
impl LauncherApp {
    fn render_left(&mut self, ui: &mut egui::Ui) {
        ui.heading(t("title", self.lang));
        ui.add_space(25.0);

        ui.vertical_centered(|ui| {
            // Arquivo .package
            let clicked = Self::render_select_button(
                ui,
                t("choose_file", self.lang),
                [220.0, 45.0],
                self.arquivo_x.is_some(),
            )
            .on_hover_text(t("choose_file_tip", self.lang))
            .clicked();
            if clicked {
                if let Some(path) = FileDialog::new()
                    .add_filter("Sims3 (.sims3pack ou .package)", &["package", "sims3pack"])
                    .pick_file()
                {
                    // define novo arquivo
                    self.arquivo_x = Some(path);

                    // 🔹 resetar resultados ao escolher novo .package
                    if self
                        .arquivo_x
                        .as_ref()
                        .and_then(|p| p.extension())
                        .map(|ext| ext == "package")
                        .unwrap_or(false)
                    {
                        self.arquivos_listados.clear();
                        self.check_done = false;
                    }
                }
            }
            if let Some(p) = &self.arquivo_x {
                if let Some(name) = p.file_name() {
                    ui.label(egui::RichText::new(name.to_string_lossy()).color(TEXT_COLOR))
                        .on_hover_text(p.display().to_string());
                }
            }

            ui.add_space(20.0);

            // Pasta Mods
            let clicked = Self::render_select_button(
                ui,
                t("mods_folder", self.lang),
                [220.0, 45.0],
                self.pasta_origem.is_some(),
            )
            .on_hover_text(t("mods_folder_tip", self.lang))
            .clicked();
            if clicked {
                if let Some(path) = FileDialog::new().pick_folder() {
                    self.pasta_origem = Some(path);
                }
            }
            if let Some(p) = &self.pasta_origem {
                ui.label(egui::RichText::new(p.display().to_string()).color(TEXT_COLOR));
            }

            ui.add_space(20.0);

            // Pasta Destino
            let clicked = Self::render_select_button(
                ui,
                t("dest_folder", self.lang),
                [220.0, 45.0],
                self.pasta_destino.is_some(),
            )
            .on_hover_text(t("dest_folder_tip", self.lang))
            .clicked();
            if clicked {
                if let Some(path) = FileDialog::new().pick_folder() {
                    self.pasta_destino = Some(path);
                }
            }
            if let Some(p) = &self.pasta_destino {
                ui.label(egui::RichText::new(p.display().to_string()).color(TEXT_COLOR));
            }

            ui.add_space(20.0);

            // Verificar CCs
            if Self::render_select_button(
                ui,
                t("verify", self.lang),
                [220.0, 50.0],
                self.check_done,
            )
            .on_hover_text(t("verify_tip", self.lang))
            .clicked()
            {
                self.processar();
            }
        });
    }

    // =============================
    // Coluna da direita
    // =============================
    fn render_right(&mut self, ui: &mut egui::Ui) {
        egui::Frame::group(ui.style())
            .fill(egui::Color32::TRANSPARENT)
            .stroke(egui::Stroke::new(1.5, egui::Color32::from_gray(180)))
            .rounding(egui::Rounding::same(10.0))
            .inner_margin(egui::Margin::same(12.0))
            .show(ui, |ui| {
                ui.set_min_height(120.0);

                if self.check_done {
                    if self.arquivos_listados.is_empty() {
                        // 🔹 já verificou, mas nada encontrado
                        ui.vertical_centered(|ui| {
                            ui.colored_label(
                                TEXT_COLOR,
                                match self.lang {
                                    Idioma::Pt => "Nenhum CC encontrado.",
                                    Idioma::Es => "Ningún CC encontrado.",
                                    Idioma::En => "No CC found.",
                                },
                            );
                        });
                    } else {
                        ui.colored_label(TEXT_COLOR, t("verified", self.lang));
                        ui.colored_label(
                            TEXT_COLOR,
                            format!("{} {}", self.arquivos_listados.len(), t("found", self.lang)),
                        );

                        ui.add_space(6.0);
                        ui.add(egui::Separator::default().spacing(8.0));
                        ui.add_space(6.0);

                        egui::ScrollArea::vertical()
                            .max_height(200.0)
                            .show(ui, |ui| {
                                for f in &self.arquivos_listados {
                                    if let Some(name) = f.file_name() {
                                        ui.label(
                                            egui::RichText::new(name.to_string_lossy())
                                                .color(TEXT_COLOR),
                                        )
                                        .on_hover_text(f.display().to_string());
                                    }
                                }
                            });

                        ui.add_space(20.0);
                        ui.horizontal(|ui| {
                            let width = ui.available_width() / 2.0 - 6.0;

                            // 🔹 Botão Exportar
                            if Self::render_select_button(
                                ui,
                                t("export", self.lang),
                                [width, 45.0],
                                self.copiar_clicked,
                            )
                            .on_hover_text(t("export_tip", self.lang))
                            .clicked()
                            {
                                self.copiar_clicked = true; // mantém verde
                                self.exportar();
                            }

                            // 🔹 Botão Compactar
                            if Self::render_select_button(
                                ui,
                                t("zip", self.lang),
                                [width, 45.0],
                                self.compactar_clicked,
                            )
                            .on_hover_text(t("zip_tip", self.lang))
                            .clicked()
                            {
                                self.compactar_clicked = true; // mantém verde
                                self.compactar();
                            }
                        });
                    }
                } else {
                    // 🔹 ainda não verificou
                    ui.vertical_centered(|ui| {
                        ui.colored_label(TEXT_COLOR, t("none", self.lang));
                    });
                }
            });
    }
}

// =============================
// MAIN
// =============================
fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_maximized(true)
            .with_resizable(false),
        ..Default::default()
    };

    eframe::run_native(
        "Veiga's S3CC Splitter",
        options,
        Box::new(|_cc| Box::new(LauncherApp::default())),
    )
}
