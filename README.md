# 中興大學學習日誌自動填寫工具

此為基於 Selenium 的自動化工具，用於 中興大學差勤系統（學習日誌），可自動登入並填寫尚未完成的學習日誌紀錄，且不會重複填寫已存在的日期。

---

### 主要特色
- 自動登入：自動處理帳號密碼輸入與登入流程。

- 智慧日期選取：自動計算指定月份的工作日（週一至週五），並排除週末。

- 重複檢查：自動抓取網頁上已存在的日誌日期，避免重複填寫。

- 瀏覽器相容性：內建瀏覽器工廠，依序嘗試啟動 Edge、Chrome、Firefox，提高穩定性。

- 民國日期轉換：自動轉換西元年為民國年格式（如 1150118）。

---

### 環境準備

```
pip install selenium webdriver-manager
```

---

### 設定檔說明 (config.json)
請在專案根目錄建立一個 config.json 檔案，內容格式如下：
```
{
  "id": "你的學號/帳號",
  "pw": "你的密碼",
  "school_value": "校內編號",
  "use_custom_date": false,
  "custom_year": 2026,
  "custom_month": 1
}
```
* id / pw: 系統登入認證。

* school_value: 系統內所屬單位的數值（Value）或名稱。

* use_custom_date:
  * false: 自動使用執行時的「當前月份」。
  * true: 使用下方指定的年份與月份。

* custom_year / custom_month: 當 use_custom_date 為 true 時生效。

---

### 使用方法
1. 確認 config.json 資訊填寫正確。

2. 確保 utils.py 與 main.py 放在同一個目錄。

3. 執行主程式：
```
python main.py
```
4.程式將會：

* 啟動瀏覽器並前往登入頁面。

* 進入「學習日誌」並切換至內嵌框架 (iframe)。

* 讀取現有日期，跳過已填寫項目。

* 逐一輸入工作內容。