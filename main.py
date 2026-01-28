from datetime import datetime
import json
from selenium import webdriver
from selenium.webdriver.chrome.service import Service
from webdriver_manager.chrome import ChromeDriverManager
from webdriver_manager.microsoft import EdgeChromiumDriverManager
from selenium.webdriver.common.by import By
from selenium.webdriver.common.keys import Keys
from selenium.webdriver.support.ui import WebDriverWait
from selenium.webdriver.support import expected_conditions as EC
from selenium.common.exceptions import WebDriverException
import time

import utils

def wait_until_browser_closes(driver):
    """
    Pauses the script and monitors the browser window.
    The script continues only after the browser is closed.
    """
    print("Program paused. Please perform manual actions. The script will resume after you close the browser...")
    
    try:
        # Keep looping as long as there is at least one window handle active
        while len(driver.window_handles) > 0:
            time.sleep(1)  # Check every second to minimize CPU usage
    except (WebDriverException, Exception):
        # Exception occurs when the connection is lost or the browser is fully closed
        pass

    print("Browser closure detected. Finalizing the process.")

def main():
    try:
        with open('config.json', 'r', encoding='utf-8') as f:
            config = json.load(f)
    except FileNotFoundError:
        print("Error: config.json not found.")
        return 
    
    your_id = config['id']
    your_pw = config['pw']
    school_value = config["school_value"]
    use_custom_date = config.get("use_custom_date", False)

    if use_custom_date:
        year = config["custom_year"]
        month = config["custom_month"]
        print(f"Using custom date: {year} / {month:02d}")
    else:
        now = datetime.now()
        year = now.year
        month = now.month
        print(f"Using current date: {year} / {month:02d}")

    print(f"{year} / {month:02d}")

    weekdays = utils.weekdays(year=year, month=month)
    print(weekdays)

    driver = utils.get_driver()
    driver.get("https://psf.nchu.edu.tw/punch/index.jsp")

    time.sleep(2)
    
    try:
        driver.find_element(By.ID, "txtLoginID").send_keys(your_id)
        driver.find_element(By.ID, "txtLoginPWD").send_keys(your_pw)
        driver.find_element(By.ID, "button").click()
        time.sleep(1)
        driver.find_element(By.LINK_TEXT, "學習日誌").click()
        wait = WebDriverWait(driver, 10)
        wait.until(EC.frame_to_be_available_and_switch_to_it((By.ID, "displayiframe")))
        print("switch to iframe!")
        existing_dates = utils.get_existing_dates(driver)
        utils.select_school_by_value(driver, school_value)
        print(f"Selected school value: {school_value}")

        for day in weekdays:

            content = utils.data_pack(day)
            work_content = content["work"]
            date_str = content["date"]
            if date_str in existing_dates:
                print(f"Skip existing date: {date_str}")
                continue

            wait = WebDriverWait(driver, 10)
            date_input = wait.until(EC.presence_of_element_located((By.ID, "date")))

            date_input = driver.find_element(By.ID, "date")
            date_input.clear()
            date_input.send_keys(date_str)
            time.sleep(0.5)

            work_input = driver.find_element(By.ID, "work")
            work_input.clear()
            work_input.send_keys(work_content)
            time.sleep(0.5)

            driver.find_element(By.ID, "btnSent").click()
            print(date_str)
            time.sleep(1)
        roc_year = year - 1911
        driver.switch_to.default_content()
        driver.find_element(By.LINK_TEXT, "學習日誌列印").click()
        wait.until(EC.frame_to_be_available_and_switch_to_it((By.ID, "displayiframe")))
        utils.select_school_by_value(driver, school_value)

        date_input = wait.until(EC.presence_of_element_located((By.ID, "dtQryBeg")))
        date_input = driver.find_element(By.ID, "dtQryBeg")
        date_input.clear()
        date_input.send_keys(f"{roc_year}{month:02d}01")
        time.sleep(0.5)                
        date_input = wait.until(EC.presence_of_element_located((By.ID, "dtQryEnd")))
        date_input = driver.find_element(By.ID, "dtQryEnd")
        date_input.clear()
        date_input.send_keys(f"{date_str}")
        time.sleep(0.5)
        driver.find_element(By.ID, "btnSent").click()

    except ValueError as e:
        print(str(e))
        driver.quit()
        return

    finally:
        wait_until_browser_closes(driver)
        driver.quit()
        print("Done!")            

if __name__ == "__main__":
    main()