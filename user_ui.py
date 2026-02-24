import tkinter as tk

windows_height = 200
windows_width = 300

windows = tk.Tk()
windows.title("Log In")
windows.geometry(f"{windows_width}x{windows_height}")
windows.resizable(False, False)

label = tk.Label(windows, text="fuck you")
label.place(x=windows_width//2, y=30,anchor="center")

windows.mainloop()