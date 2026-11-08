#![cfg_attr(
    all(target_os = "windows", not(debug_assertions)),
    windows_subsystem = "windows"
)]

use std::path::PathBuf;

use anyhow::{Context, Result, bail};
use eframe::egui;
use nchu_diary_autofill::{config, gui::DiaryApp};

fn main() -> Result<()> {
    let mut path: Option<PathBuf> = None;
    let mut demo = false;
    let mut args = std::env::args_os().skip(1);
    while let Some(arg) = args.next() {
        match arg.to_str() {
            Some("--demo") => demo = true,
            Some("--config") => {
                path = Some(args.next().context("--config 後面需要設定檔路徑")?.into())
            }
            Some("--help" | "-h") => {
                println!(
                    "NCHU Diary AutoFill\n\n用法：nchu-diary-autofill [--config 路徑] [--demo]\n\n--config  指定相容 Python 版的 config.json\n--demo    離線示範，不登入、不寫入設定或送出紀錄"
                );
                return Ok(());
            }
            _ => bail!("未知參數：{}；使用 --help 查看用法", arg.to_string_lossy()),
        }
    }
    let path = match path {
        Some(path) => path,
        None => config::default_path()?,
    };
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1120.0, 760.0])
            .with_min_inner_size([1000.0, 640.0]),
        renderer: eframe::Renderer::Glow,
        ..Default::default()
    };
    eframe::run_native(
        "NCHU Diary AutoFill",
        options,
        Box::new(move |cc| {
            cc.egui_ctx.set_visuals(egui::Visuals::light());
            cc.egui_ctx.global_style_mut(|style| {
                for text in [egui::TextStyle::Body, egui::TextStyle::Button] {
                    style
                        .text_styles
                        .insert(text, egui::FontId::proportional(16.0));
                }
                style
                    .text_styles
                    .insert(egui::TextStyle::Heading, egui::FontId::proportional(22.0));
                style
                    .text_styles
                    .insert(egui::TextStyle::Small, egui::FontId::proportional(13.0));
                style.spacing.interact_size.y = 30.0;
                style.spacing.button_padding = egui::vec2(10.0, 5.0);
                style.spacing.item_spacing = egui::vec2(8.0, 8.0);
            });
            Ok(Box::new(DiaryApp::new(&cc.egui_ctx, path, demo)?))
        }),
    )
    .map_err(|error| anyhow::anyhow!("無法啟動桌面介面：{error}"))
}
