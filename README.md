# Antrian Poliklinik (Antri-Poli)

Sistem antrian poliklinik berbasis web yang modern dan responsif, dibangun menggunakan **Rust (Axum)** untuk performa tinggi dan **HTML/JS** untuk interface yang interaktif. Sistem ini dirancang untuk diintegrasikan dengan database SIMRS (seperti Khanza) atau berjalan mandiri dalam mode demo.

## ✨ Fitur Utama

-   **Display Antrian Real-time**: Menampilkan antrian pasien yang sedang dipanggil dan yang sedang menunggu.
-   **Lokalisasi Bahasa Indonesia**: Format tanggal dan waktu menggunakan nama hari dan bulan dalam Bahasa Indonesia.
-   **Mode Demo & Produksi**: Dapat dijalankan tanpa database (mode demo) atau terhubung ke MySQL (mode produksi).
-   **Pembersihan Otomatis**: Menghapus data antrian lama secara otomatis setiap pergantian hari.
-   **Manajemen Pemanggilan**: Interface kontrol untuk memanggil pasien berdasarkan poliklinik dan dokter.
-   **Responsif UI**: Interface yang bersih dan mudah dibaca untuk monitor TV di area poliklinik.

## 🛠️ Tech Stack

-   **Backend**: [Rust](https://www.rust-lang.org/) dengan framework [Axum](https://github.com/tokio-rs/axum).
-   **Database**: MySQL (via [SQLx](https://github.com/launchbadge/sqlx)).
-   **Frontend**: Vanilla HTML5, CSS3, dan JavaScript.
-   **Utilities**: Tokio (Asynchronous runtime), Chrono (Time support), Serde (Serialization).

## 🚀 Cara Menjalankan

### Persiapan
1.  Pastikan Anda telah menginstal **Rust** dan **Cargo**.
2.  Clone repository ini:
    ```bash
    git clone https://github.com/ghazurti/Antrian-Poliklinik.git
    cd Antrian-Poliklinik
    ```

### Konfigurasi (.env)
Buat file `.env` di root direktori:
```env
DATABASE_URL=mysql://user:password@localhost:3306/database_name
PORT=3030
DEMO_MODE=false # Set ke true untuk mencoba tanpa database
```

### Menjalankan Server
```bash
cargo run
```
Akses di browser:
-   **Display TV**: `http://localhost:3030/`
-   **Kontrol Panggilan**: `http://localhost:3030/static/control.html`

## 📂 Struktur Proyek

-   `src/main.rs`: Logika utama backend, API, dan integrasi database.
-   `static/index.html`: Interface display antrian untuk pasien.
-   `static/control.html`: Interface pemanggilan untuk petugas/dokter.
-   `static/app.js`: Logika frontend dan polling data antrian.

## 📄 Lisensi

Proyek ini dikembangkan untuk mempermudah alur pelayanan di unit kesehatan. Silakan gunakan dan kembangkan lebih lanjut.
