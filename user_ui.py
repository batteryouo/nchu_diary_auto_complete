from datetime import date, timedelta

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
        tk.Label(self.root, text="Student ID:").pack(pady=(10, 0))
        self.entry_id = tk.Entry(self.root)
        self.entry_id.pack(pady=5)

        tk.Label(self.root, text="Password:").pack(pady=(10, 0))
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
    def __init__(self, year, month):
        super().__init__("Select Dates", "350x450")
        self.selected_dates = []
        
        tk.Label(self.root, text=f"請勾選要執行的日期 ({year}/{month:02d}):", font=("Arial", 10)).pack(pady=10)
        self.start_date = tk.StringVar(value=f"{year}-{month:02d}-01")

        frame_top = tk.Frame(self.root)
        frame_top.pack(pady=5)

        self.use_recent_15 = tk.BooleanVar(value=False)
        chk = tk.Checkbutton(frame_top, text="使用最近15個工作日", variable=self.use_recent_15)
        chk.grid(row=0, column=0, columnspan=2, pady=5)

 
        tk.Label(frame_top, text="起始日:").grid(row=1, column=0)
        self.entry_start = tk.Entry(frame_top, textvariable=self.start_date, width=15)
        self.entry_start.grid(row=1, column=1)

        # Create a scrollable listbox for multiple selection
        frame = tk.Frame(self.root)
        frame.pack(pady=5, fill="both", expand=True)

        scrollbar = tk.Scrollbar(frame, orient="vertical")
        self.listbox = tk.Listbox(frame, selectmode="multiple", yscrollcommand=scrollbar.set, font=("Courier New", 10))
        
        scrollbar.config(command=self.listbox.yview)
        scrollbar.pack(side="right", fill="y")
        self.listbox.pack(side="left", fill="both", expand=True, padx=(10, 0))

        # Generate weekdays for the given month
        _, num_days = calendar.monthrange(year, month)
        for day in range(1, num_days + 1):
            date_obj = date(year, month, day)
            # Only add weekdays (0-4 = Mon-Fri)
            if date_obj.weekday() < 5:
                # Format: 2026-02-01 (Mon)
                display_str = date_obj.strftime("%Y-%m-%d (%a)")
                self.listbox.insert(tk.END, display_str)

        tk.Button(self.root, text="確認選擇", command=self.submit).pack(pady=15)
        self.root.mainloop()

    def submit(self):
        if self.use_recent_15.get():
            try:
                start = date.fromisoformat(self.start_date.get())
            except Exception:
                messagebox.showwarning("Warning", "日期格式錯誤 (YYYY-MM-DD)")
                return

            today = date.today()
            result = []
            current = today

            while len(result) < 15 and current >= start:
                if current.weekday() < 5:
                    result.append(current.strftime("%Y-%m-%d"))
                current -= timedelta(days=1)

            self.selected_dates = list(reversed(result))

        else:
            indices = self.listbox.curselection()
            self.selected_dates = [self.listbox.get(i).split(" ")[0] for i in indices]

            if not self.selected_dates:
                messagebox.showwarning("Warning", "請至少選擇一天")
                return

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

def run_date_multi_select_ui(year, month):
    app = DateMultiSelectUI(year, month)
    return app.success, app.selected_dates
