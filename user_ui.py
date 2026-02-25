import tkinter as tk
from tkinter import messagebox, ttk
import json
import os

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
        super().__init__("NCHU Login", "300x200")
        tk.Label(self.root, text="Student ID:").pack(pady=(10, 0))
        self.entry_id = tk.Entry(self.root)
        self.entry_id.pack(pady=5)

        tk.Label(self.root, text="Password:").pack(pady=(10, 0))
        self.entry_pw = tk.Entry(self.root, show="*")
        self.entry_pw.pack(pady=5)

        tk.Button(self.root, text="Login", command=self.submit).pack(pady=20)
        
        # Load default values
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

        # Set default value
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

def run_login_ui():
    app = LoginUI()
    return app.success

def run_school_select_ui(options):
    app = SchoolSelectUI(options)
    return app.success