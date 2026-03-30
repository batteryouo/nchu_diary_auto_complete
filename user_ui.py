from datetime import date, timedelta
import holidays

import tkinter as tk
from tkinter import messagebox, ttk
from tkcalendar import Calendar
import json
import os
import calendar

class BaseUI:
    def __init__(self, title, geometry):
        self.root = tk.Tk()
        self.root.title(title)
        self.root.geometry(geometry)
        self.config_file = 'config.json'
        self.success = False

    def load_config_data(self):
        if os.path.exists(self.config_file):
            with open(self.config_file, 'r', encoding='utf-8') as f:
                return json.load(f)
        return {}

    def save_config_data(self, new_data):
        config_data = self.load_config_data()
        config_data.update(new_data)
        with open(self.config_file, 'w', encoding='utf-8') as f:
            json.dump(config_data, f, indent=4, ensure_ascii=False)

class LoginUI(BaseUI):
    def __init__(self):
        super().__init__("NCHU Login", "300x250")
        tk.Label(self.root, text="身分證字號:").pack(pady=(10, 0))
        self.entry_id = tk.Entry(self.root)
        self.entry_id.pack(pady=5)

        tk.Label(self.root, text="出生年月日(e.g. 900928):").pack(pady=(10, 0))
        self.entry_pw = tk.Entry(self.root, show="*")
        self.entry_pw.pack(pady=5)

        # Logic: Checkbox to decide whether to skip subsequent UIs
        self.force_manual_var = tk.BooleanVar(value=False)
        self.chk_manual = tk.Checkbutton(
            self.root, 
            text="不使用預設設定檔", 
            variable=self.force_manual_var
        )
        self.chk_manual.pack(pady=5)

        tk.Button(self.root, text="Login", command=self.submit).pack(pady=10)
        
        # Result placeholder
        self.force_manual_result = False
        
        config = self.load_config_data()
        self.entry_id.insert(0, config.get('id', ''))
        self.entry_pw.insert(0, config.get('pw', ''))
        self.root.mainloop()

    def submit(self):
        if not self.entry_id.get() or not self.entry_pw.get():
            messagebox.showwarning("Warning", "Fields cannot be empty")
            return
        self.save_config_data({'id': self.entry_id.get(), 'pw': self.entry_pw.get()})
        self.success = True
        self.force_manual_result = self.force_manual_var.get()
        self.root.destroy()

class SchoolSelectUI(BaseUI):
    def __init__(self, options):
        super().__init__("Select School Value", "350x150")
        self.options = options if options else ["--select--"]
        
        tk.Label(self.root, text="Select School Number:").pack(pady=(10, 0))
        self.selected_val = tk.StringVar()
        self.combo = ttk.Combobox(self.root, textvariable=self.selected_val, values=self.options, state="readonly")
        self.combo.pack(pady=5)

        tk.Button(self.root, text="Confirm", command=self.submit).pack(pady=15)

        config = self.load_config_data()
        saved_val = config.get('school_value', '')
        if saved_val in self.options:
            self.selected_val.set(saved_val)
        else:
            self.selected_val.set(self.options[0])
            
        self.root.mainloop()

    def submit(self):
        val = self.selected_val.get()
        if val == "--select--":
            messagebox.showwarning("Warning", "Please select a valid value")
            return
        self.save_config_data({'school_value': val})
        self.success = True
        self.root.destroy()

class DateSelectUI(BaseUI):
    def __init__(self):
        # Increased height for calendar widget
        super().__init__("Select Target Date", "400x450")
        
        tk.Label(self.root, text="請選擇目標日期 (程式將提取該月份):", font=("Arial", 10)).pack(pady=10)
        
        # Calendar widget
        self.cal = Calendar(self.root, selectmode='day', cursor="hand2")
        self.cal.pack(pady=10, padx=10, fill="both", expand=True)

        tk.Button(self.root, text="確認日期", command=self.submit).pack(pady=20)
        
        self.selected_year = None
        self.selected_month = None
        self.root.mainloop()

    def submit(self):
        # Extract date object from calendar
        date_obj = self.cal.selection_get()
        self.selected_year = date_obj.year
        self.selected_month = date_obj.month
        
        # Save to config
        self.save_config_data({
            "custom_year": self.selected_year,
            "custom_month": self.selected_month,
            "use_custom_date": True
        })
        
        self.success = True
        self.root.destroy()

class DateMultiSelectUI(BaseUI):
    def __init__(self, year, month, existing_dates):
        super().__init__("Select Dates", "600x500")
        self.year = year
        self.month = month
        self.existing_dates = existing_dates # ROC format strings
        self.selected_dates = []

        # Initialize Taiwan holidays
        self.tw_holidays = holidays.Taiwan(years=self.year)
        self.skip_holiday_var = tk.BooleanVar(value=True)
        
        # UI Layout: Left (Calendar), Right (Controls)
        main_frame = tk.Frame(self.root)
        main_frame.pack(fill="both", expand=True, padx=10, pady=10)

        # 1. Calendar (Left)
        self.cal = Calendar(self.root, selectmode='day', 
                            year=self.year, month=self.month,
                            cursor="hand2")
        self.cal.pack(side="left", fill="both", expand=True, padx=5)

        # 2. Control Panel (Right)
        control_frame = tk.Frame(self.root)
        control_frame.pack(side="right", fill="y", padx=5)
        # Auto-select settings
        self.auto_select_var = tk.BooleanVar(value=True)
        tk.Checkbutton(control_frame, text="Auto-select N days", 
                       variable=self.auto_select_var).pack(anchor="w")

        tk.Label(control_frame, text="N (Days):").pack(anchor="w")
        self.n_days_entry = tk.Entry(control_frame, width=10)
        self.n_days_entry.insert(0, "15")
        self.n_days_entry.pack(anchor="w", pady=5)

        self.skip_weekend_var = tk.BooleanVar(value=True)
        tk.Checkbutton(control_frame, text="Skip Weekends", variable=self.skip_weekend_var).pack(anchor="w")
        tk.Checkbutton(control_frame, text="Skip Public Holidays", variable=self.skip_holiday_var).pack(anchor="w")
        # Selection Display
        tk.Label(control_frame, text="Current Selected:").pack(anchor="w", pady=(10, 0))
        self.selection_label = tk.Label(control_frame, text="0 days", fg="blue")
        self.selection_label.pack(anchor="w")

        # Buttons
        tk.Button(control_frame, text="Apply Auto Logic", command=self.apply_auto_logic, 
                  bg="#e1e1e1").pack(fill="x", pady=5)
        
        tk.Button(control_frame, text="Confirm & Submit", command=self.submit, 
                  bg="#4CAF50", fg="white").pack(fill="x", side="bottom", pady=10)

        # Manual Click Binding
        self.cal.bind("<<CalendarSelected>>", self.on_date_click)
        
        # Bind month change event
        self.cal.bind("<<CalendarMonthChanged>>", self.on_month_changed)

        # Initial logic trigger
        if self.auto_select_var.get():
            self.apply_auto_logic()
        
        self.root.mainloop()

    def on_date_click(self, event=None):
        """Toggle manual date selection"""
        selected_date = self.cal.selection_get()
        iso_str = selected_date.strftime("%Y-%m-%d")
        
        if iso_str in self.selected_dates:
            self.selected_dates.remove(iso_str)
            self.cal.calevent_remove(date=selected_date)
        else:
            self.selected_dates.append(iso_str)
            self.cal.calevent_create(selected_date, 'selected', 'selected')
            self.cal.tag_config('selected', background='red', foreground='white')
        
        self.update_display()
    def on_month_changed(self, event):
        """Update logic when user navigates to a different month/year in the calendar"""
        # Get the displayed month and year from the calendar widget
        displayed_date = self.cal.get_displayed_month() # returns (month, year)
        self.month = displayed_date[0]
        self.year = displayed_date[1]
        
        print(f"Calendar view changed to: {self.year}/{self.month:02d}")
        
        # Optional: Auto-re-run logic when month changes
        if self.auto_select_var.get():
            self.apply_auto_logic()

    def apply_auto_logic(self):
        """Automatically pick dates based on rules, including existing dates in the count."""
        # Clear current UI selection highlights
        for d_str in self.selected_dates:
            self.cal.calevent_remove(date=date.fromisoformat(d_str))
        self.selected_dates = []

        try:
            n_target = int(self.n_days_entry.get())
        except ValueError:
            messagebox.showerror("Error", "N must be a number")
            return

        skip_weekend = self.skip_weekend_var.get()
        skip_holiday = self.skip_holiday_var.get()
        _, num_days = calendar.monthrange(self.year, self.month)
        
        # This counter tracks both existing logs and new selections
        total_count = 0
        
        for day in range(1, num_days + 1):
            if total_count >= n_target:
                break
            
            cur_date = date(self.year, self.month, day)
            roc_str = f"{cur_date.year-1911:03d}{cur_date.month:02d}{cur_date.day:02d}"
            
            # Check if it's a weekend
            is_weekend = cur_date.weekday() >= 5
            if skip_weekend and is_weekend:
                continue
            # 2. Check Public Holiday
            if skip_holiday and cur_date in self.tw_holidays:
                print(f"Skipping holiday: {cur_date} ({self.tw_holidays.get(cur_date)})")
                continue
            
            # If the date already exists in the system:
            # We count it towards the N limit, but don't add it to 'selected_dates' 
            # because main.py doesn't need to submit it again.
            if roc_str in self.existing_dates:
                total_count += 1
                continue
                
            # If it's a valid new date to fill
            iso_str = cur_date.strftime("%Y-%m-%d")
            self.selected_dates.append(iso_str)
            self.cal.calevent_create(cur_date, 'selected', 'selected')
            total_count += 1
            
        self.cal.tag_config('selected', background='red', foreground='white')
        self.update_display()

    def update_display(self):
        self.selection_label.config(text=f"{len(self.selected_dates)} days")

    def submit(self):
        if not self.selected_dates and not self.auto_select_var.get():
            if not messagebox.askyesno("Warning", "No dates selected. Proceed anyway?"):
                return

        # Update the config to match the calendar's current view
        self.save_config_data({
            "custom_year": self.year,
            "custom_month": self.month,
            "use_custom_date": True  # Force use of the UI-selected month
        })
        
        self.success = True
        self.root.destroy()

def run_login_ui():
    app = LoginUI()
    # Return both login success and the "Force Manual" flag
    return app.success, app.force_manual_result

def run_school_select_ui(options):
    app = SchoolSelectUI(options)
    return app.success

def run_date_select_ui():
    app = DateSelectUI()
    return app.success, app.selected_year, app.selected_month

def run_date_multi_select_ui(year, month, existing_dates):
    app = DateMultiSelectUI(year, month, existing_dates)
    return app.success, app.selected_dates, app.year, app.month
