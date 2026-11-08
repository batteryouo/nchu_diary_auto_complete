use std::{collections::BTreeSet, path::Path, time::Duration};

use anyhow::{Context, Result, bail, ensure};
use chrono::{Local, NaiveDate};
use thirtyfour::{common::print::PrintParameters, components::SelectElement, prelude::*};
use tokio::time::{Instant, sleep};

use crate::{
    config::Config,
    dates::{Month, WORK_CONTENT, parse_roc, roc_date},
};

pub const LOGIN_URL: &str = "https://psf.nchu.edu.tw/punch/index.jsp";
const WAIT: Duration = Duration::from_secs(15);
const POLL: Duration = Duration::from_millis(150);

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct School {
    pub value: String,
    pub label: String,
}

pub fn preferred_school(
    options: &[School],
    saved: Option<&str>,
    force_manual: bool,
) -> Option<String> {
    if options.len() == 1 {
        return Some(options[0].value.clone());
    }
    if force_manual {
        return None;
    }
    saved
        .and_then(|saved| {
            options
                .iter()
                .find(|option| option.value == saved || option.label == saved)
        })
        .map(|option| option.value.clone())
}

pub struct BrowserSession {
    pub driver: WebDriver,
}

impl BrowserSession {
    /// Drivers are resolved/downloaded by thirtyfour; no Python/Selenium server is required.
    pub async fn launch(mut progress: impl FnMut(String)) -> Result<Self> {
        let mut failures = Vec::new();
        for name in ["Edge", "Chrome", "Firefox"] {
            progress(format!("正在啟動 {name}（首次使用需下載驅動程式）…"));
            match Self::launch_named(name, false).await {
                Ok(session) => return Ok(session),
                Err(error) => {
                    progress(format!("{name} 無法啟動，嘗試下一個瀏覽器。"));
                    failures.push(format!("{name}: {error:#}"));
                }
            }
        }
        bail!(
            "無法啟動瀏覽器。請確認已安裝 Edge、Chrome 或 Firefox，並可連線下載驅動程式。\n{}",
            failures.join("\n")
        )
    }

    /// Also used by the local integration test; headless sessions never touch the school site.
    pub async fn launch_named(name: &str, headless: bool) -> Result<Self> {
        let capabilities: Capabilities = match name {
            "Edge" => {
                let mut caps = DesiredCapabilities::edge();
                caps.add_arg(if headless {
                    "--headless=new"
                } else {
                    "--start-maximized"
                })?;
                caps.into()
            }
            "Chrome" => {
                let mut caps = DesiredCapabilities::chrome();
                caps.add_arg(if headless {
                    "--headless=new"
                } else {
                    "--start-maximized"
                })?;
                caps.into()
            }
            "Firefox" => {
                let mut caps = DesiredCapabilities::firefox();
                if headless {
                    caps.add_arg("-headless")?;
                }
                caps.into()
            }
            _ => bail!("不支援的瀏覽器：{name}"),
        };
        let mut managed = WebDriver::managed(capabilities);
        if let Some(cache) = std::env::var_os("NCHU_DRIVER_CACHE") {
            managed = managed.cache_dir(cache.into());
        }
        let driver = managed.await?;
        driver
            .set_page_load_timeout(Duration::from_secs(30))
            .await?;
        driver.set_script_timeout(WAIT).await?;
        Ok(Self { driver })
    }

    async fn find(&self, by: By) -> Result<WebElement> {
        Ok(self.driver.query(by).wait(WAIT, POLL).single().await?)
    }

    async fn replace(&self, id: &str, value: &str) -> Result<WebElement> {
        let element = self.find(By::Id(id)).await?;
        element.clear().await?;
        element.send_keys(value).await?;
        Ok(element)
    }

    async fn open_section(&self, text: &str) -> Result<()> {
        self.driver.enter_default_frame().await?;
        self.find(By::LinkText(text)).await?.click().await?;
        self.find(By::Id("displayiframe"))
            .await?
            .enter_frame()
            .await?;
        self.find(By::Name("schno")).await?;
        Ok(())
    }

    pub async fn login(&self, config: &Config, url: &str) -> Result<Vec<School>> {
        self.driver.goto(url).await.context("無法開啟登入頁面")?;
        self.replace("txtLoginID", &config.id).await?;
        self.replace("txtLoginPWD", &config.pw).await?;
        self.find(By::Id("button")).await?.click().await?;
        self.open_section("學習日誌")
            .await
            .context("登入失敗或找不到學習日誌，請檢查登入資料")?;
        self.schools().await
    }

    pub async fn schools(&self) -> Result<Vec<School>> {
        let element = self.find(By::Name("schno")).await?;
        let select = SelectElement::new(&element).await?;
        let mut options = Vec::new();
        for option in select.options().await? {
            if let Some(value) = option
                .attr("value")
                .await?
                .filter(|value| !value.is_empty())
            {
                let label = option.text().await?;
                options.push(School {
                    label: if label.trim().is_empty() {
                        value.clone()
                    } else {
                        label
                    },
                    value,
                });
            }
        }
        ensure!(!options.is_empty(), "此帳號沒有可選擇的計畫單位");
        Ok(options)
    }

    pub async fn select_school(&self, school: &str) -> Result<()> {
        let element = self.find(By::Name("schno")).await?;
        let select = SelectElement::new(&element).await?;
        let options = self.schools().await?;
        let option = options
            .iter()
            .find(|option| option.value == school)
            .or_else(|| options.iter().find(|option| option.label.trim() == school))
            .context("儲存的計畫單位已失效，請重新選擇")?;
        // Selecting an option dispatches change, including the site's form-navigation handler.
        select.select_by_value(&option.value).await?;
        let deadline = Instant::now() + WAIT;
        loop {
            // Re-find after onchange: the original element may belong to the old document.
            if let Ok(current) = self.driver.find(By::Name("schno")).await
                && current.value().await.ok().flatten().as_deref() == Some(option.value.as_str())
                && self
                    .driver
                    .execute("return document.readyState === 'complete'", Vec::new())
                    .await?
                    .convert::<bool>()?
            {
                return Ok(());
            }
            ensure!(Instant::now() < deadline, "等待計畫切換逾時");
            sleep(POLL).await;
        }
    }

    pub async fn existing_dates(&self) -> Result<BTreeSet<NaiveDate>> {
        self.find(By::Id("date")).await?;
        let mut dates = BTreeSet::new();
        for cell in self
            .driver
            .find_all(By::Css("td.footable-first-column"))
            .await?
        {
            // textContent also reads Footable rows hidden on another client-side page.
            if let Some(text) = cell.prop("textContent").await?
                && let Some(date) = parse_roc(&text)
            {
                dates.insert(date);
            }
        }
        Ok(dates)
    }

    pub async fn prepare_school(&self, school: &str) -> Result<BTreeSet<NaiveDate>> {
        self.select_school(school).await?;
        self.existing_dates().await
    }

    pub async fn submit_date(&self, school: &str, date: NaiveDate) -> Result<()> {
        ensure!(
            date <= Local::now().date_naive(),
            "不可填寫今天之後的日期：{date}"
        );
        // The site's POST resets the school dropdown to its first option. Re-select it
        // before every submission so a later date cannot be saved under another plan.
        self.select_school(school).await?;
        let roc = roc_date(date);
        let input = self.replace("date", &roc).await?;
        self.replace("work", WORK_CONTENT).await?;
        ensure!(
            self.find(By::Name("schno"))
                .await?
                .value()
                .await?
                .as_deref()
                == Some(school),
            "校內編號在送出前被重設，已停止以避免寫入錯誤計畫"
        );
        self.find(By::Id("btnSent")).await?.click().await?;
        // Never retry POSTs: a timeout can occur after the server has already saved the row.
        input
            .wait_until()
            .wait(WAIT, POLL)
            .stale()
            .await
            .with_context(|| format!("{roc} 已送出，但無法確認頁面更新；請檢查瀏覽器中的紀錄"))?;
        let deadline = Instant::now() + WAIT;
        loop {
            if self.existing_dates().await?.contains(&date) {
                return Ok(());
            }
            ensure!(
                Instant::now() < deadline,
                "{roc} 已送出，但紀錄未出現在表格；請人工確認，程式不會重複送出"
            );
            sleep(POLL).await;
        }
    }

    pub async fn open_report(&self, school: &str, month: Month, today: NaiveDate) -> Result<()> {
        self.open_section("學習日誌列印").await?;
        self.select_school(school).await?;
        let (start, end) = month.report_range(today);
        self.replace("dtQryBeg", &roc_date(start)).await?;
        self.replace("dtQryEnd", &roc_date(end)).await?;
        ensure!(
            self.find(By::Name("schno"))
                .await?
                .value()
                .await?
                .as_deref()
                == Some(school),
            "校內編號在報表送出前被重設，已停止以避免查詢錯誤計畫"
        );
        let existing_windows = self.driver.windows().await?;
        self.find(By::Id("btnSent")).await?.click().await?;
        self.switch_to_report_window(&existing_windows).await?;
        self.wait_for_print_report().await?;
        Ok(())
    }

    async fn switch_to_report_window(&self, existing_windows: &[WindowHandle]) -> Result<()> {
        let deadline = Instant::now() + WAIT;
        loop {
            let windows = self.driver.windows().await?;
            if let Some(handle) = windows
                .iter()
                .find(|handle| !existing_windows.contains(handle))
            {
                self.driver.switch_to_window(handle.clone()).await?;
                self.driver.enter_default_frame().await?;
                return Ok(());
            }
            // Some browser versions navigate the current tab instead of opening a new one.
            let current_page_is_report = self
                .driver
                .execute(
                    "return document.body && (document.body.innerText.includes('國立中興大學計畫人員學習日誌') || document.querySelector('#printed') !== null);",
                    Vec::new(),
                )
                .await?
                .convert::<bool>()?;
            if current_page_is_report {
                self.driver.enter_default_frame().await?;
                return Ok(());
            }
            ensure!(Instant::now() < deadline, "等待學習日誌列印分頁開啟逾時");
            sleep(POLL).await;
        }
    }

    async fn wait_for_print_report(&self) -> Result<()> {
        let deadline = Instant::now() + WAIT;
        loop {
            let ready = self
                .driver
                .execute(
                    "return document.readyState === 'complete' && document.body && (document.body.innerText.includes('國立中興大學計畫人員學習日誌') || document.querySelector('#printed') !== null);",
                    Vec::new(),
                )
                .await?
                .convert::<bool>()?;
            if ready {
                return Ok(());
            }
            ensure!(Instant::now() < deadline, "等待學習日誌列印報表載入逾時");
            sleep(POLL).await;
        }
    }

    pub async fn save_current_pdf(&self, path: &Path) -> Result<()> {
        let pdf = self
            .driver
            .print_page(PrintParameters::default())
            .await
            .context("無法將報表轉成 PDF")?;
        tokio::fs::write(path, pdf)
            .await
            .with_context(|| format!("無法儲存 PDF：{}", path.display()))?;
        Ok(())
    }

    pub async fn is_open(&self) -> bool {
        self.driver
            .windows()
            .await
            .is_ok_and(|windows| !windows.is_empty())
    }

    pub async fn close(self) {
        let _ = self.driver.quit().await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn school_selection_validates_saved_values_and_manual_override() {
        let options = vec![
            School {
                value: "A".into(),
                label: "計畫 A".into(),
            },
            School {
                value: "B".into(),
                label: "計畫 B".into(),
            },
        ];
        assert_eq!(
            preferred_school(&options, Some("B"), false).as_deref(),
            Some("B")
        );
        assert_eq!(
            preferred_school(&options, Some("計畫 B"), false).as_deref(),
            Some("B")
        );
        assert_eq!(preferred_school(&options, Some("deleted"), false), None);
        assert_eq!(preferred_school(&options, Some("B"), true), None);
        assert_eq!(
            preferred_school(&options[..1], None, true).as_deref(),
            Some("A")
        );
        assert_eq!(preferred_school(&[], None, false), None);
    }
}
