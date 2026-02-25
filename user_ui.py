import tkinter as tk
from tkinter import messagebox
import json
import os

class LoginUI:
    def __init__(self, root):
        self.root = root
        self.root.title("NCHU Login System")
        self.root.geometry("300x250")
        self.config_file = 'config.json'
        self.success = False

        # UI Elements
        tk.Label(root, text="Student ID:").pack(pady=(20, 0))
        self.entry_id = tk.Entry(root)
        self.entry_id.pack(pady=5)

        tk.Label(root, text="Password:").pack(pady=(10, 0))
        self.entry_pw = tk.Entry(root, show="*")
        self.entry_pw.pack(pady=5)

        self.btn_login = tk.Button(root, text="Save & Run", command=self.save_and_exit)
        self.btn_login.pack(pady=20)

        self.load_config()

    def load_config(self):
        if os.path.exists(self.config_file):
            try:
                with open(self.config_file, 'r', encoding='utf-8') as f:
                    config = json.load(f)
                    self.entry_id.insert(0, config.get('id', ''))
                    self.entry_pw.insert(0, config.get('pw', ''))
            except Exception as e:
                print(f"Error loading config: {e}")

    def save_and_exit(self):
        user_id = self.entry_id.get()
        user_pw = self.entry_pw.get()

        if not user_id or not user_pw:
            messagebox.showwarning("Warning", "Please enter both ID and Password")
            return

        # Update config.json
        config_data = {}
        if os.path.exists(self.config_file):
            with open(self.config_file, 'r', encoding='utf-8') as f:
                config_data = json.load(f)
        
        config_data['id'] = user_id
        config_data['pw'] = user_pw

        with open(self.config_file, 'w', encoding='utf-8') as f:
            json.dump(config_data, f, indent=4, ensure_ascii=False)

        self.success = True
        self.root.destroy()

def run_login_ui():
    """Function to be called by main.py"""
    root = tk.Tk()
    app = LoginUI(root)
    root.mainloop()
    return app.success