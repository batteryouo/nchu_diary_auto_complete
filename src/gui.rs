use std::{collections::BTreeSet, path::PathBuf};

use anyhow::Result;
use chrono::{Datelike, Local, NaiveDate};
use eframe::egui::{self, Color32, RichText};

use crate::{
    browser::School,
    config::Config,
    dates::{MIN_YEAR, Month, SelectionRules, TaiwanHolidays, WORK_CONTENT, auto_select},
    worker::{Command, Event, Worker},
};

#[derive(PartialEq, Eq)]
enum Page {
    Login,
    Busy,
    School,
    Dates,
    Report,
    Error,
}

pub struct DiaryApp {
    worker: Worker,
    config_path: PathBuf,
    config: Config,
    config_error: Option<String>,
    demo: bool,
    force_manual: bool,
    page: Page,
    status: String,
    error: String,
    history: Vec<String>,
    schools: Vec<School>,
    school_choice: String,
    school: String,
    month: Month,
    holidays: TaiwanHolidays,
    rules: SelectionRules,
    auto: bool,
    existing: BTreeSet<NaiveDate>,
    selected: BTreeSet<NaiveDate>,
    submitted: usize,
    skipped: usize,
    report_pdf: Option<PathBuf>,
    closing: bool,
    restarting: bool,
    stopped: bool,
    compact_mode: bool,
    applied_compact_mode: Option<bool>,
}

impl DiaryApp {
    pub fn new(ctx: &egui::Context, config_path: PathBuf, demo: bool) -> Result<Self> {
        let (config, config_error) = if demo {
            (
                Config {
                    id: "DEMO".into(),
                    pw: "demo".into(),
                    ..Default::default()
                },
                None,
            )
        } else {
            match Config::load(&config_path) {
                Ok(config) => (config, None),
                Err(error) => (Config::default(), Some(format!("{error:#}"))),
            }
        };
        let repaint = ctx.clone();
        let worker = Worker::spawn(config_path.clone(), demo, move || repaint.request_repaint())?;
        Ok(Self {
            worker,
            config_path,
            config,
            config_error,
            demo,
            force_manual: false,
            page: Page::Login,
            status: String::new(),
            error: String::new(),
            history: Vec::new(),
            schools: Vec::new(),
            school_choice: String::new(),
            school: String::new(),
            month: Month::of(Local::now().date_naive()),
            holidays: TaiwanHolidays::bundled()?,
            rules: SelectionRules::default(),
            auto: true,
            existing: BTreeSet::new(),
            selected: BTreeSet::new(),
            submitted: 0,
            skipped: 0,
            report_pdf: None,
            closing: false,
            restarting: false,
            stopped: false,
            compact_mode: false,
            applied_compact_mode: None,
        })
    }

    fn send(&mut self, command: Command, status: &str) {
        if !self.demo
            && matches!(
                &command,
                Command::Login { .. } | Command::SelectSchool(_) | Command::Submit { .. }
            )
        {
            self.compact_mode = true;
        }
        match self.worker.send(command) {
            Ok(()) => {
                self.status = status.into();
                self.page = Page::Busy;
            }
            Err(error) => {
                self.error = error.to_string();
                self.page = Page::Error;
            }
        }
    }

    fn apply_rules(&mut self) {
        self.selected = auto_select(
            self.month,
            &self.rules,
            &self.existing,
            &self.holidays,
            Local::now().date_naive(),
        );
    }

    fn change_month(&mut self, month: Month) {
        let month = month.min(Month::of(Local::now().date_naive()));
        if self.month != month {
            self.month = month;
            self.selected.clear();
            if self.auto {
                self.apply_rules();
            }
        }
    }

    fn events(&mut self, ctx: &egui::Context) {
        let events: Vec<_> = self.worker.events.try_iter().collect();
        for event in events {
            match event {
                Event::Progress(status) => {
                    self.status.clone_from(&status);
                    self.history.push(status);
                    if self.history.len() > 100 {
                        self.history.remove(0);
                    }
                }
                Event::ChooseSchool(schools) => {
                    self.compact_mode = false;
                    self.school_choice = schools
                        .iter()
                        .find(|school| Some(&school.value) == self.config.school_value.as_ref())
                        .or_else(|| schools.first())
                        .map(|school| school.value.clone())
                        .unwrap_or_default();
                    self.schools = schools;
                    self.page = Page::School;
                }
                Event::DatesReady { school, existing } => {
                    self.compact_mode = false;
                    self.config.school_value = Some(school.clone());
                    self.school = school;
                    self.existing = existing;
                    self.selected.clear();
                    if self.auto {
                        self.apply_rules();
                    }
                    self.page = Page::Dates;
                }
                Event::ReportReady {
                    submitted,
                    skipped,
                    pdf_path,
                } => {
                    self.compact_mode = !self.demo;
                    self.submitted = submitted;
                    self.skipped = skipped;
                    self.report_pdf = pdf_path;
                    self.page = Page::Report;
                }
                Event::Failed(error) => {
                    self.compact_mode = false;
                    self.error = error;
                    self.page = Page::Error;
                }
                Event::BrowserClosed => {
                    self.compact_mode = false;
                    if self.page == Page::Report && !self.demo {
                        self.closing = true;
                        self.worker.shutdown();
                    } else {
                        self.page = Page::Login;
                        self.status = "瀏覽器已關閉，可重新登入。".into();
                    }
                }
                Event::Stopped => {
                    self.stopped = true;
                    if self.closing {
                        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                    }
                }
            }
        }
        if self.stopped && self.restarting && !self.closing {
            let repaint = ctx.clone();
            self.restarting = false;
            match Worker::spawn(self.config_path.clone(), self.demo, move || {
                repaint.request_repaint()
            }) {
                Ok(worker) => {
                    self.worker = worker;
                    self.stopped = false;
                    self.compact_mode = false;
                    self.page = Page::Login;
                    self.status = "工作已取消，瀏覽器已關閉。".into();
                }
                Err(error) => {
                    self.error = error.to_string();
                    self.page = Page::Error;
                }
            }
        }
    }

    fn cancel(&mut self) {
        self.compact_mode = !self.demo;
        self.restarting = true;
        self.status = "正在取消工作並關閉瀏覽器…".into();
        self.worker.shutdown();
        self.page = Page::Busy;
    }

    fn apply_window_mode(&mut self, ctx: &egui::Context) {
        if self.applied_compact_mode == Some(self.compact_mode) {
            return;
        }

        ctx.send_viewport_cmd(egui::ViewportCommand::Maximized(false));
        if self.compact_mode {
            ctx.send_viewport_cmd(egui::ViewportCommand::MinInnerSize(egui::vec2(
                420.0, 220.0,
            )));
            ctx.send_viewport_cmd(egui::ViewportCommand::MaxInnerSize(egui::vec2(
                520.0, 360.0,
            )));
            ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(egui::vec2(480.0, 300.0)));
            ctx.send_viewport_cmd(egui::ViewportCommand::Resizable(false));
            ctx.send_viewport_cmd(egui::ViewportCommand::WindowLevel(
                egui::WindowLevel::AlwaysOnTop,
            ));
            ctx.send_viewport_cmd(egui::ViewportCommand::OuterPosition(egui::pos2(24.0, 24.0)));
        } else {
            ctx.send_viewport_cmd(egui::ViewportCommand::MaxInnerSize(egui::vec2(
                10_000.0, 10_000.0,
            )));
            ctx.send_viewport_cmd(egui::ViewportCommand::MinInnerSize(egui::vec2(
                1000.0, 640.0,
            )));
            ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(egui::vec2(1120.0, 760.0)));
            ctx.send_viewport_cmd(egui::ViewportCommand::Resizable(true));
            ctx.send_viewport_cmd(egui::ViewportCommand::WindowLevel(
                egui::WindowLevel::Normal,
            ));
        }
        ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
        self.applied_compact_mode = Some(self.compact_mode);
    }

    fn compact_ui(&mut self, ui: &mut egui::Ui) {
        ui.heading(match self.page {
            Page::Report => "報表已開啟",
            Page::Error => "工作停止",
            _ => "自動填寫進行中",
        });
        ui.separator();
        ui.add_space(8.0);

        match self.page {
            Page::Report => {
                ui.label(format!(
                    "新增 {} 筆，跳過 {} 筆已有紀錄。",
                    self.submitted, self.skipped
                ));
                ui.colored_label(
                    Color32::from_rgb(164, 99, 0),
                    "請在瀏覽器確認資料與列印報表；此狀態窗會保持在最上層。",
                );
                if let Some(path) = &self.report_pdf {
                    ui.label(format!("PDF 已儲存：{}", path.display()));
                }
                if ui.button("關閉瀏覽器並結束本次工作").clicked() {
                    self.send(Command::CloseBrowser, "正在關閉瀏覽器…");
                    // Preserve Report state so BrowserClosed can finish the application.
                    self.page = Page::Report;
                }
            }
            Page::Error => {
                ui.colored_label(Color32::DARK_RED, &self.error);
                if ui.button("關閉瀏覽器並返回登入").clicked() {
                    self.cancel();
                }
            }
            _ => {
                ui.horizontal(|ui| {
                    ui.spinner();
                    ui.label(&self.status);
                });
                ui.colored_label(
                    Color32::from_rgb(164, 99, 0),
                    "瀏覽器正在自動操作，請不要手動輸入或關閉瀏覽器。",
                );
                if !self.closing && !self.restarting && ui.button("取消並返回登入").clicked()
                {
                    self.cancel();
                }
            }
        }

        ui.add_space(8.0);
        ui.separator();
        ui.small("需要選擇計畫、日期或處理錯誤時，完整視窗會自動恢復。");
    }

    fn login_ui(&mut self, ui: &mut egui::Ui) {
        ui.heading("登入");
        ui.label("輸入中興大學學習日誌系統使用的登入資料。");
        ui.add_space(12.0);
        egui::Grid::new("login")
            .spacing([16.0, 14.0])
            .show(ui, |ui| {
                ui.label("身分證字號");
                ui.add(
                    egui::TextEdit::singleline(&mut self.config.id)
                        .min_size(egui::vec2(260.0, 30.0)),
                );
                ui.end_row();
                ui.label("出生年月日");
                ui.add(
                    egui::TextEdit::singleline(&mut self.config.pw)
                        .min_size(egui::vec2(260.0, 30.0))
                        .password(true)
                        .hint_text("例如：900928"),
                );
                ui.end_row();
            });
        ui.add_space(8.0);
        ui.checkbox(
            &mut self.force_manual,
            "重新選擇計畫單位（不使用已儲存的計畫）",
        );
        ui.small("登入資料會記在本機 config.json，沿用原版格式。");
        if !self.demo {
            ui.small("開始後瀏覽器會自動操作；左上角的置頂狀態窗會持續顯示進度。");
        }
        if let Some(error) = self.config_error.clone() {
            ui.colored_label(Color32::DARK_RED, error);
            ui.label("請修復設定檔後重新讀取，或以 --config 指定其他檔案。");
            if ui.button("重新讀取設定檔").clicked() {
                match Config::load(&self.config_path) {
                    Ok(config) => {
                        self.config = config;
                        self.config_error = None;
                    }
                    Err(error) => self.config_error = Some(format!("{error:#}")),
                }
            }
        }
        ui.add_space(12.0);
        let valid = self.config_error.is_none()
            && !self.config.id.trim().is_empty()
            && !self.config.pw.is_empty();
        if ui
            .add_enabled(
                valid,
                egui::Button::new(if self.demo {
                    "開始示範"
                } else {
                    "登入並開啟瀏覽器"
                }),
            )
            .clicked()
        {
            self.history.clear();
            self.error.clear();
            self.send(
                Command::Login {
                    config: self.config.clone(),
                    force_manual: self.force_manual,
                },
                "正在準備瀏覽器…",
            );
        }
        if !self.status.is_empty() {
            ui.label(&self.status);
        }
    }

    fn school_ui(&mut self, ui: &mut egui::Ui) {
        ui.heading("選擇計畫單位");
        ui.label("選擇要填寫日誌的校內編號。");
        egui::ComboBox::from_id_salt("school")
            .width(440.0)
            .selected_text(
                self.schools
                    .iter()
                    .find(|school| school.value == self.school_choice)
                    .map(|school| school.label.as_str())
                    .unwrap_or("請選擇"),
            )
            .show_ui(ui, |ui| {
                for school in &self.schools {
                    ui.selectable_value(
                        &mut self.school_choice,
                        school.value.clone(),
                        &school.label,
                    );
                }
            });
        ui.add_space(12.0);
        if ui
            .add_enabled(
                !self.school_choice.is_empty(),
                egui::Button::new("確認計畫"),
            )
            .clicked()
        {
            self.send(
                Command::SelectSchool(self.school_choice.clone()),
                "正在切換計畫並讀取紀錄…",
            );
        }
        if ui.button("取消並返回登入").clicked() {
            self.cancel();
        }
    }

    fn calendar_ui(&mut self, ui: &mut egui::Ui) {
        let today = Local::now().date_naive();
        let current_month = Month::of(today);
        ui.horizontal(|ui| {
            if ui.button("< 上個月").clicked() {
                self.change_month(self.month.shift(-1));
            }
            let mut year = self.month.year;
            let mut month = self.month.month;
            ui.add(
                egui::DragValue::new(&mut year)
                    .range(MIN_YEAR..=today.year())
                    .suffix(" 年"),
            );
            let last_month = if year == today.year() {
                today.month()
            } else {
                12
            };
            month = month.min(last_month);
            egui::ComboBox::from_id_salt("month")
                .selected_text(format!("{month} 月"))
                .show_ui(ui, |ui| {
                    for value in 1..=last_month {
                        ui.selectable_value(&mut month, value, format!("{value} 月"));
                    }
                });
            if let Ok(next) = Month::new(year, month) {
                self.change_month(next);
            }
            if ui
                .add_enabled(self.month < current_month, egui::Button::new("下個月 >"))
                .clicked()
            {
                self.change_month(self.month.shift(1));
            }
        });
        ui.add_space(12.0);
        let days: Vec<_> = self.month.days().collect();
        let offset = days[0].weekday().num_days_from_monday() as usize;
        let width = ((ui.available_width() - 48.0) / 7.0).max(40.0);
        egui::Grid::new("calendar")
            .spacing([5.0, 5.0])
            .show(ui, |ui| {
                for label in ["一", "二", "三", "四", "五", "六", "日"] {
                    ui.label(label);
                }
                ui.end_row();
                for cell in 0_usize..42 {
                    if let Some(date) = cell
                        .checked_sub(offset)
                        .and_then(|index| days.get(index))
                        .copied()
                    {
                        let existing = self.existing.contains(&date);
                        let future = date > today;
                        let mut button = egui::Button::new(format!(
                            "{}{}",
                            date.day(),
                            if existing { "\n已有" } else { "" }
                        ))
                        .min_size(egui::vec2(width, 48.0))
                        .selected(self.selected.contains(&date));
                        if existing {
                            button = button.fill(Color32::from_rgb(207, 228, 218));
                        } else if self.holidays.contains(date) {
                            button = button.fill(Color32::from_rgb(251, 228, 208));
                        }
                        let response = ui.add_enabled(!existing && !future, button);
                        if response.clicked() && !self.selected.remove(&date) {
                            self.selected.insert(date);
                        }
                        response.on_hover_text(format!(
                            "{date}{}{}",
                            if existing {
                                "\n此計畫已有紀錄"
                            } else if future {
                                "\n未來日期不可填寫"
                            } else {
                                ""
                            },
                            self.holidays
                                .name(date)
                                .map(|name| format!("\n{name}"))
                                .unwrap_or_default()
                        ));
                    } else {
                        ui.allocate_space(egui::vec2(width, 48.0));
                    }
                    if cell % 7 == 6 {
                        ui.end_row();
                    }
                }
            });
        ui.small("綠色：已有紀錄　橘色：國定假日　藍色或外框：本次選取　灰色停用：未來日期");
    }

    fn dates_ui(&mut self, ui: &mut egui::Ui) {
        ui.heading("選擇填寫日期");
        ui.label(format!("計畫：{}", self.school));
        ui.add_space(8.0);
        ui.columns(2, |columns| {
            self.calendar_ui(&mut columns[0]);
            let ui = &mut columns[1];
            ui.checkbox(&mut self.auto, "切換月份時自動選取 N 天");
            ui.horizontal(|ui| {
                ui.label("目標天數 N：");
                ui.add(egui::DragValue::new(&mut self.rules.target_days).range(0..=31));
            });
            ui.checkbox(&mut self.rules.skip_weekends, "跳過週末");
            ui.checkbox(&mut self.rules.skip_holidays, "跳過國定假日");
            ui.small("N 包含符合上述條件的已有紀錄；今天之後的日期會強制鎖定。");
            if ui.button("套用自動選取規則").clicked() {
                self.apply_rules();
            }
            if ui.button("清除選取").clicked() {
                self.selected.clear();
            }
            ui.small("可點選日曆增減日期；手動選取不受週末及假日篩選限制。");
            ui.separator();
            ui.label(RichText::new(format!("本次新增：{} 天", self.selected.len())).strong());
            ui.label(format!(
                "本月已有：{} 天",
                self.existing
                    .iter()
                    .filter(|date| self.month.contains(**date))
                    .count()
            ));
            ui.label(format!("工作內容：{WORK_CONTENT}"));
            let (start, end) = self.month.report_range(Local::now().date_naive());
            ui.label(format!("報表區間：{start} ～ {end}"));
            egui::ScrollArea::vertical()
                .id_salt("selected")
                .max_height(130.0)
                .show(ui, |ui| {
                    for date in &self.selected {
                        ui.label(date.to_string());
                    }
                });
            if self.selected.is_empty() {
                ui.label("沒有新增日期，確認後直接查詢報表。");
            }
            if ui
                .button(if self.demo {
                    "示範填寫與報表"
                } else if self.selected.is_empty() {
                    "僅開啟報表"
                } else {
                    "確認並送出"
                })
                .clicked()
            {
                self.send(
                    Command::Submit {
                        month: self.month,
                        dates: self.selected.clone(),
                    },
                    "正在確認已有紀錄…",
                );
            }
            if ui.button("取消並返回登入").clicked() {
                self.cancel();
            }
        });
    }

    fn content(&mut self, ui: &mut egui::Ui) {
        if self.compact_mode {
            self.compact_ui(ui);
            return;
        }
        ui.heading("NCHU 學習日誌自動填寫");
        if self.demo {
            ui.colored_label(
                Color32::from_rgb(164, 99, 0),
                "示範模式：使用虛擬資料，不會連線學校網站或儲存登入資料。",
            );
        }
        ui.separator();
        ui.add_space(12.0);
        match self.page {
            Page::Login => self.login_ui(ui),
            Page::School => self.school_ui(ui),
            Page::Dates => self.dates_ui(ui),
            Page::Busy => {
                ui.spinner();
                ui.label(&self.status);
                if !self.closing && !self.restarting && ui.button("取消並返回登入").clicked()
                {
                    self.cancel();
                }
            }
            Page::Report => {
                ui.heading(if self.demo {
                    "示範完成"
                } else {
                    "報表已開啟"
                });
                ui.label(format!(
                    "新增 {} 筆，跳過 {} 筆已有紀錄。",
                    self.submitted, self.skipped
                ));
                ui.label(if self.demo {
                    "這是示範結果，未連線或新增真實日誌。"
                } else {
                    "PDF 已自動儲存；請在瀏覽器確認資料與列印報表，手動關閉瀏覽器後程式會結束。"
                });
                if let Some(path) = &self.report_pdf {
                    ui.small(format!("PDF 路徑：{}", path.display()));
                }
                if ui
                    .button(if self.demo {
                        "返回登入"
                    } else {
                        "關閉瀏覽器並結束本次工作"
                    })
                    .clicked()
                {
                    self.send(Command::CloseBrowser, "正在關閉瀏覽器…");
                    // Preserve Report state so BrowserClosed can finish the application.
                    self.page = Page::Report;
                }
            }
            Page::Error => {
                ui.heading("工作停止");
                ui.colored_label(Color32::DARK_RED, &self.error);
                ui.label(
                    "若曾送出日期，請先在瀏覽器確認已儲存的紀錄。重新登入會重新讀取已有日期。",
                );
                if ui.button("關閉瀏覽器並返回登入").clicked() {
                    self.cancel();
                }
            }
        }
        if !self.history.is_empty() {
            ui.add_space(12.0);
            egui::CollapsingHeader::new("執行紀錄").show(ui, |ui| {
                egui::ScrollArea::vertical()
                    .id_salt("history")
                    .max_height(140.0)
                    .stick_to_bottom(true)
                    .show(ui, |ui| {
                        for line in &self.history {
                            ui.label(line);
                        }
                    });
            });
        }
        ui.add_space(12.0);
        ui.separator();
        ui.small(format!("設定檔：{}", self.config_path.display()));
    }
}

impl eframe::App for DiaryApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        self.events(&ctx);
        self.apply_window_mode(&ctx);
        if ctx.input(|input| input.viewport().close_requested()) && !self.stopped {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            self.closing = true;
            self.status = "正在關閉瀏覽器與背景工作…".into();
            self.page = Page::Busy;
            self.worker.shutdown();
        }
        egui::CentralPanel::default().show(ui, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| self.content(ui));
        });
    }
}
