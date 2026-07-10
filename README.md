# NCHU Learning Log Automator

這是一個專為中興大學學生設計的自動化工具，用於快速填寫學習日誌並產出報表。

---

### 核心功能
- 圖形化登入介面：提供 GUI 輸入學號與密碼，並支援記住資訊。

- 智慧日期選取：
  - 多選日曆：可自由點選本月要填寫的日期。
  - 自動選取 N 天：預設自動選取 15 天，省去逐一核對的時間。
  - 過濾機制：自動跳過週末（可選）以及網頁上已經存在紀錄的日期。

- 自動化填寫：自動登入、切換框架 (Frame)、選取計畫單位並填入工作內容。

- 報表自動產出：填寫完成後自動跳轉至列印頁面，並根據「自定義月份」或「當前日期」自動設定報表區間。

- 多瀏覽器支援：系統會依序嘗試啟動 Edge、Chrome 或 Firefox。

---

### 快速開始（一般使用者，不需要裝 Python）

1. 取得 `NCHU_Diary_AutoFill.exe`（跟有打包過的人索取，或自行依照下方「打包成 exe」步驟產生），放到你想要的資料夾（`config.json` 會自動產生在同一個資料夾）。
2. 雙擊執行，依照下方「使用說明」操作即可。
3. 如果 Windows Defender 或防毒軟體跳出警告（PyInstaller 打包的 exe 沒有簽章，常被誤判），選擇「仍要執行」。

---

### 檔案結構

- main.py: 程式進入點，負責整體的執行流程邏輯。

- user_ui.py: 包含所有的 GUI 視窗類別（登入、校號選擇、日期多選）。

- utils.py: 存放瀏覽器驅動設定、日期轉換（民國年/西元年）及網頁元素爬取等工具函式。

- config.json: 儲存使用者設定、學號與計畫編號（自動生成）。

### 使用環境

- Python 3.x
- 必要套件：
```
pip install selenium webdriver-manager tkcalendar holidays
```

---

### 打包成 exe

#### 方法一：GitHub Actions 自動建置（推薦，不需要在本機安裝 Python/uv）

專案已設定 `.github/workflows/build.yml`：

- 推上 `v*` 開頭的 tag（例如 `git tag v1.0.0 && git push origin v1.0.0`）：GitHub 會自動建置並把 `NCHU_Diary_AutoFill.exe` 附加到對應的 Release，直接到 Releases 頁面下載即可。
- 也可以到 GitHub 專案的 Actions 頁籤，手動觸發 `Build exe` workflow（workflow_dispatch），完成後在該次執行的 Artifacts 下載 exe，不會建立 Release。

整個下載依賴、打包的過程都在 GitHub 的雲端機器上執行，本機不需要裝 Python 或 uv。

#### 方法二：本機手動打包（開發者，需要先裝 [uv](https://docs.astral.sh/uv/)）

```
uv sync
uv run pyinstaller NCHU_Diary_AutoFill.spec
```

打包完成後，執行檔會在 `dist/NCHU_Diary_AutoFill.exe`，可直接分享給其他使用者雙擊執行。

---

### 使用說明

1. 執行 python main.py。

2. 在彈出的登入視窗輸入學號與密碼。
     - 若想更改已儲存的計畫單位，可勾選「不使用預設設定檔」。

3. 進入日期選取頁面：
  
    - 設定想要填寫的天數（預設 15 天）。
  
    - 確認是否跳過六日。
  
    - 點擊 "Apply Auto Logic" 進行預選，或直接在日曆上手動點選日期。

4. 點擊 "Confirm & Submit"，程式將自動開啟瀏覽器執行填寫動作。

5. 手動確認與關閉：程式填寫並跳轉至列印報表後會暫停，請確認資料無誤後，手動關閉瀏覽器視窗，程式才會正式結束。