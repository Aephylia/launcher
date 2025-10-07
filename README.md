# 💫 Aephylia Launcher

A simple command-line OG Fortnite launcher.

---

## 🚀 Usage

Run the command with arguments:
```bash
aephylia-launcher.exe --path path --email email --pass password
```

---

## ⚙️ Command-Line Arguments

| Argument | Type | Description |
|-----------|------|-------------|
| `--path` | string | Path to the Fortnite installation folder |
| `--email` | string | E-mail used on log on |
| `--pass` | string | Password used on log on |

---

## 🧠 Notes
- For now, gameserver.dll and redirect.dll are forced and hardcoded.
- Tested on 10.40, with Reload Backend + Cobalt SSL

---

## 📄 Compiling
- Ensure you have Rust installed.
```bash
cargo build
```
