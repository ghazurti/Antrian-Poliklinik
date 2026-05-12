"""
Generate audio clips untuk sistem antrian poli.
Jalankan sekali: python3 generate_audio.py
"""
from gtts import gTTS
import os

OUTPUT_DIR = "static/audio"
os.makedirs(OUTPUT_DIR, exist_ok=True)

clips = {
    # Frasa tetap
    "mohon_perhatian": "Mohon perhatian",
    "nomor":           "Nomor",
    "silakan_menuju":  "silakan menuju",
    "dipanggil":       "dipanggil",

    # Digit 0–9
    "0": "nol",
    "1": "satu",
    "2": "dua",
    "3": "tiga",
    "4": "empat",
    "5": "lima",
    "6": "enam",
    "7": "tujuh",
    "8": "delapan",
    "9": "sembilan",
}

for name, text in clips.items():
    path = os.path.join(OUTPUT_DIR, f"{name}.mp3")
    tts = gTTS(text=text, lang="id", slow=False)
    tts.save(path)
    print(f"  ✔ {path}  ← \"{text}\"")

print(f"\n✅ {len(clips)} file audio berhasil dibuat di {OUTPUT_DIR}/")
