import calendar
from datetime import date, timedelta
import re
import os

from selenium import webdriver
from selenium.webdriver.chrome.service import Service as ChromeService
from webdriver_manager.chrome import ChromeDriverManager

from selenium.webdriver.common.by import By
from selenium.webdriver.edge.service import Service as EdgeService
from selenium.webdriver.edge.options import Options as EdgeOptions
from webdriver_manager.microsoft import EdgeChromiumDriverManager
from selenium.webdriver.firefox.service import Service as FirefoxService
from webdriver_manager.firefox import GeckoDriverManager

from selenium.webdriver.support.ui import Select

os.environ['WDM_SSL_VERIFY'] = '0'
os.environ['WDM_TIMEOUT'] = '5'

def weekdays(year, month):
    dayList = []

    _, num_days = calendar.monthrange(year, month)
    today = date.today()
    count = 0
    for day in range(1, num_days + 1):
        current_date = date(year, month, day)
        if current_date.weekday() < 5 and current_date < today:
            dayList.append(current_date)
            count += 1

        if count > 16:
            break

    return dayList

def data_pack(input_date: date):
    roc_year = input_date.year - 1911

    work_content = "協助教授計畫"
    date_str = f"{roc_year:03d}{input_date.month:02d}{input_date.day:02d}"

    returnDict = {"date": date_str, "work": work_content}
    return returnDict

def get_driver():
    """Browser factory: Edge -> Chrome -> Firefox -> Others."""
    
    # 1. Try Microsoft Edge
    try:
        print("Attempting to start Microsoft Edge...")
        options = EdgeOptions()
        options.add_argument("--start-maximized")
        service = EdgeService()
        return webdriver.Edge(service=service, options=options)
    except Exception as e:
        print(f"Edge failed: {e}")

    # 2. Try Google Chrome
    try:
        print("Attempting to start Google Chrome...")
        return webdriver.Chrome(service=ChromeService(ChromeDriverManager().install()))
    except Exception as e:
        print(f"Chrome failed: {e}")

    # 3. Try Mozilla Firefox
    try:
        print("Attempting to start Mozilla Firefox...")
        return webdriver.Firefox(service=FirefoxService(GeckoDriverManager().install()))
    except Exception as e:
        print(f"Firefox failed: {e}")

    # 4. Fallback to system default (Others)
    try:
        print("Attempting to start system default driver...")
        return webdriver.Chrome() 
    except Exception as e:
        print("All browser attempts failed.")
        return None


def get_existing_dates(driver):
    """
    Read existing log dates from table.
    Only keep valid ROC date strings (7 digits).
    """

    existing_dates = set()
    pattern = re.compile(r"^\d{7}$")

    cells = driver.find_elements(By.CSS_SELECTOR, "td.footable-first-column")

    for cell in cells:
        text = cell.text.strip()
        if pattern.match(text):
            existing_dates.add(text)

    return existing_dates

def select_school_by_value(driver, school_value):
    """
    Select school number by value or visible text.
    Raise exception if not found.
    """

    select_element = driver.find_element(By.NAME, "schno")
    select = Select(select_element)

    # Try select by value
    for option in select.options:
        if option.get_attribute("value") == school_value:
            select.select_by_value(school_value)
            return

    # Try select by visible text (fallback)
    for option in select.options:
        if option.text.strip() == school_value:
            select.select_by_visible_text(school_value)
            return

    raise ValueError(f"School value not found: {school_value}")