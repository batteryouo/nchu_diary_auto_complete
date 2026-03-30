import calendar
from datetime import datetime
import json
import os

from selenium.webdriver.common.by import By
from selenium.webdriver.support.ui import WebDriverWait
from selenium.webdriver.support import expected_conditions as EC
from selenium.common.exceptions import WebDriverException
import time

import utils
import user_ui
from selenium.webdriver.support.ui import Select

CONFIG_FILE = "config.json"

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
  # ===== 1. Login UI =====
  login_success, force_manual = user_ui.run_login_ui()

  if not login_success:
    print("Exit: Login UI closed.")
    return

  # ===== 2. Load config =====
  try:
    with open(CONFIG_FILE, 'r', encoding='utf-8') as f:
      config = json.load(f)
  except FileNotFoundError:
    print(f"Error: {CONFIG_FILE} not found.")
    return

  use_custom_date = config.get("use_custom_date", False)

  if use_custom_date:
    year = config["custom_year"]
    month = config["custom_month"]
    print(f"Using custom date: {year}/{month:02d}")
  else:
    now = datetime.now()
    year = now.year
    month = now.month
    print(f"Using current date: {year}/{month:02d}")

  # ===== 3. Start browser =====
  driver = utils.get_driver()
  driver.get("https://psf.nchu.edu.tw/punch/index.jsp")

  time.sleep(2)

  try:
    # ===== 4. Login =====
    driver.find_element(By.ID, "txtLoginID").send_keys(config['id'])
    driver.find_element(By.ID, "txtLoginPWD").send_keys(config['pw'])
    driver.find_element(By.ID, "button").click()

    time.sleep(1)

    driver.find_element(By.LINK_TEXT, "學習日誌").click()

    wait = WebDriverWait(driver, 10)
    wait.until(EC.frame_to_be_available_and_switch_to_it((By.ID, "displayiframe")))

    # ===== 5. School selection =====
    select_element = driver.find_element(By.NAME, "schno")
    select_obj = Select(select_element)

    available_options = [
      opt.get_attribute("value")
      for opt in select_obj.options
      if opt.get_attribute("value")
    ]

    has_school_data = "school_value" in config and config["school_value"]

    if len(available_options) == 1:
      config["school_value"] = available_options[0]
      with open(CONFIG_FILE, 'w', encoding='utf-8') as f:
        json.dump(config, f, indent=4, ensure_ascii=False)

    elif force_manual or not has_school_data:
      if not user_ui.run_school_select_ui(available_options):
        print("Selection cancelled.")
        return

      with open(CONFIG_FILE, 'r', encoding='utf-8') as f:
        config = json.load(f)

    else:
      print("Using saved school value")

    # ===== 6. Get existing dates =====
    existing_dates = utils.get_existing_dates(driver)

    school_value = config["school_value"]
    utils.select_school_by_value(driver, school_value)

    print(f"Selected school: {school_value}")

    # ===== 7. Date selection UI  =====
    # Passing year, month, and the list of already filled dates to the UI
    success, selected_dates = user_ui.run_date_multi_select_ui(year, month, existing_dates)

    if not success:
        print("Date selection cancelled")
        return
    # ===== 8. Fill data =====
    last_processed_date = None
    
    # Process only if selected_dates is not empty
    if selected_dates:
        selected_dates.sort()
        for date_str_iso in selected_dates:
            day_obj = datetime.strptime(date_str_iso, "%Y-%m-%d").date()
            content = utils.data_pack(day_obj)
            work_content = content["work"]
            date_str = content["date"]

            if date_str in existing_dates:
                print(f"Skip existing: {date_str}")
                continue

            date_input = wait.until(EC.presence_of_element_located((By.ID, "date")))
            date_input.clear()
            date_input.send_keys(date_str)

            work_input = driver.find_element(By.ID, "work")
            work_input.clear()
            work_input.send_keys(work_content)

            driver.find_element(By.ID, "btnSent").click()
            print(f"Submitted: {date_str}")
            last_processed_date = date_str
            time.sleep(1)
    else:
        print("No new dates selected. Proceeding to report generation.")

    # ===== 9. Print report =====
    roc_year = year - 1911
    driver.switch_to.default_content()
    driver.find_element(By.LINK_TEXT, "學習日誌列印").click()

    wait.until(EC.frame_to_be_available_and_switch_to_it((By.ID, "displayiframe")))
    utils.select_school_by_value(driver, school_value)

    # 1. Start date = First day of the target month
    start_input = driver.find_element(By.ID, "dtQryBeg")
    start_input.clear()
    start_input.send_keys(f"{roc_year}{month:02d}01")

    # 2. End date logic:
    # If custom date is used, end at the last day of that month.
    # If current date is used, end at today.
    end_input = driver.find_element(By.ID, "dtQryEnd")
    end_input.clear()

    if use_custom_date:
        _, last_day = calendar.monthrange(year, month)
        report_end_str = f"{roc_year}{month:02d}{last_day:02d}"
    else:
        now = datetime.now()
        report_end_str = f"{now.year - 1911}{now.month:02d}{now.day:02d}"

    end_input.send_keys(report_end_str)
    print(f"Generating report from {roc_year}{month:02d}01 to {report_end_str}")

    driver.find_element(By.ID, "btnSent").click()

  except ValueError as e:
    print(str(e))
    
    import traceback
    traceback.print_exc()

    driver.quit()
    return

  finally:
    wait_until_browser_closes(driver)
    driver.quit()
    print("Done!")           

if __name__ == "__main__":
    main()