use axum::{
    extract::{Query, State},
    http::StatusCode,
    response::{Html, IntoResponse, Json},
    routing::{get, post},
    Router,
};
use chrono::Local;
use serde::{Deserialize, Serialize};
use serde_json::json;
use sqlx::mysql::MySqlPool;
use std::sync::{
    atomic::{AtomicU64, Ordering},
    Arc,
};
use tower_http::{cors::CorsLayer, services::ServeDir};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

// ─── State ───────────────────────────────────────────────────────────────────

struct AppState {
    db: MySqlPool,
    /// Increment setiap kali tabel antripoli berubah (INSERT/DELETE/re-call)
    call_seq: AtomicU64,
    /// UPDATE_TIME terakhir dari antripoli (untuk deteksi re-call)
    last_table_ts: tokio::sync::Mutex<String>,
}

// ─── Models ──────────────────────────────────────────────────────────────────

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
struct AntrianItem {
    no_antrian: String,
    nama_poli: String,
    nama_pasien: String,
    status: String, // "DIPANGGIL" | "MENUNGGU" | "SELESAI"
    #[sqlx(default)]
    ts_panggilan: String,
    #[sqlx(default)]
    no_rawat: String,
    #[sqlx(default)]
    kd_dokter: String,
    #[sqlx(default)]
    kd_poli: String,
}

#[derive(Debug, Serialize)]
struct AntrianResponse {
    sedang_dipanggil: Vec<AntrianItem>,
    menunggu: Vec<AntrianItem>,
    tanggal: String,
    jam: String,
    /// Bertambah setiap tabel antripoli berubah — frontend pakai ini untuk deteksi re-call
    call_seq: u64,
    /// Nomor antrian pasien yang paling terakhir dipanggil (berdasarkan mutasi_berkas.diterima)
    last_called_no_antrian: Option<String>,
}
#[derive(Debug, Serialize, sqlx::FromRow)]
struct PoliItem {
    kd_poli: String,
    nm_poli: String,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
struct JadwalItem {
    nm_dokter: String,
    nm_poli: String,
    jam_mulai: String,
    jam_selesai: String,
}

#[derive(Debug, Deserialize)]
struct PanggilRequest {
    no_rawat: String,
    kd_dokter: String,
    kd_poli: String,
}

#[derive(Debug, Deserialize)]
struct ListPasienQuery {
    kd_poli: String,
}

// ─── Handlers ────────────────────────────────────────────────────────────────

async fn index() -> impl IntoResponse {
    Html(include_str!("../static/index.html"))
}

// ─── Helpers ─────────────────────────────────────────────────────────────────

fn format_tanggal_indo(now: chrono::DateTime<chrono::Local>) -> String {
    let day = match now.format("%u").to_string().as_str() {
        "1" => "Senin",
        "2" => "Selasa",
        "3" => "Rabu",
        "4" => "Kamis",
        "5" => "Jumat",
        "6" => "Sabtu",
        "7" => "Minggu",
        _ => "",
    };
    let month = match now.format("%m").to_string().as_str() {
        "01" => "Januari",
        "02" => "Februari",
        "03" => "Maret",
        "04" => "April",
        "05" => "Mei",
        "06" => "Juni",
        "07" => "Juli",
        "08" => "Agustus",
        "09" => "September",
        "10" => "Oktober",
        "11" => "November",
        "12" => "Desember",
        _ => "",
    };
    format!(
        "{}, {} {} {}",
        day,
        now.format("%d"),
        month,
        now.format("%Y")
    )
}

async fn get_antrian(
    State(state): State<Arc<AppState>>,
) -> Result<Json<AntrianResponse>, (StatusCode, String)> {
    // reg_periksa menggunakan no_rkm_medis (bukan no_rm) untuk link ke pasien
    let rows = sqlx::query_as::<_, AntrianItem>(
        r#"
        SELECT
            rp.no_reg AS no_antrian,
            CONCAT(
                COALESCE(po.nm_poli, ap.kd_poli),
                ' - ',
                COALESCE(d.nm_dokter, ap.kd_dokter)
            ) AS nama_poli,
            COALESCE(ps.nm_pasien, ap.no_rawat) AS nama_pasien,
            CASE ap.status
                WHEN '1' THEN 'DIPANGGIL'
                WHEN '2' THEN 'DIPANGGIL'
                ELSE 'MENUNGGU'
            END AS status,
            CAST(mb.diterima AS CHAR) as ts_panggilan,
            ap.no_rawat,
            ap.kd_dokter,
            ap.kd_poli
        FROM antripoli ap
        LEFT JOIN reg_periksa rp ON ap.no_rawat     = rp.no_rawat
        LEFT JOIN poliklinik  po ON ap.kd_poli      = po.kd_poli
        LEFT JOIN dokter       d ON ap.kd_dokter    = d.kd_dokter
        LEFT JOIN pasien      ps ON rp.no_rkm_medis = ps.no_rkm_medis
        LEFT JOIN mutasi_berkas mb ON ap.no_rawat   = mb.no_rawat
        WHERE ap.status IN ('0', '1', '2')
          AND ap.no_rawat LIKE CONCAT(DATE_FORMAT(CURDATE(), '%Y/%m/%d'), '/%')
          AND COALESCE(po.nm_poli, '') NOT LIKE '%IGD%'
        ORDER BY
            ts_panggilan DESC,
            ap.kd_poli ASC
        LIMIT 50
        "#,
    )
    .fetch_all(&state.db)
    .await
    .map_err(|e| {
        tracing::error!("DB error: {}", e);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Database error: {}", e),
        )
    })?;

    // ─── Deteksi perubahan tabel antripoli (termasuk re-call) ─────────────────
    // MySQL UPDATE_TIME berubah setiap DELETE/INSERT, termasuk panggil-ulang.
    let table_ts: Option<String> = sqlx::query_scalar(
        "SELECT CAST(UPDATE_TIME AS CHAR) FROM INFORMATION_SCHEMA.TABLES \
         WHERE TABLE_SCHEMA = DATABASE() AND TABLE_NAME = 'antripoli'",
    )
    .fetch_optional(&state.db)
    .await
    .unwrap_or(None)
    .flatten();

    let ts_str = table_ts.unwrap_or_default();
    let call_seq = {
        let mut last = state.last_table_ts.lock().await;
        if *last != ts_str {
            *last = ts_str;
            state.call_seq.fetch_add(1, Ordering::SeqCst);
        }
        state.call_seq.load(Ordering::SeqCst)
    };

    let now = Local::now();
    let tanggal = format_tanggal_indo(now);
    let jam = now.format("%H:%M:%S").to_string();

    let sedang_dipanggil: Vec<AntrianItem> = rows
        .iter()
        .filter(|r| r.status == "DIPANGGIL")
        .map(|r| AntrianItem {
            no_antrian: r.no_antrian.clone(),
            nama_poli: r.nama_poli.clone(),
            nama_pasien: r.nama_pasien.clone(),
            status: r.status.clone(),
            ts_panggilan: r.ts_panggilan.clone(),
            no_rawat: r.no_rawat.clone(),
            kd_dokter: r.kd_dokter.clone(),
            kd_poli: r.kd_poli.clone(),
        })
        .collect();

    let menunggu: Vec<AntrianItem> = rows
        .into_iter()
        .filter(|r| r.status == "MENUNGGU")
        .collect();

    // Ambil nomor antrian yang paling terakhir masuk/diupdate (untuk panggil spesifik)
    let last_called_no_antrian = sedang_dipanggil.first().map(|i| i.no_antrian.clone());

    Ok(Json(AntrianResponse {
        sedang_dipanggil,
        menunggu,
        tanggal,
        jam,
        call_seq,
        last_called_no_antrian,
    }))
}

async fn get_antrian_demo() -> Json<AntrianResponse> {
    let now = Local::now();
    Json(AntrianResponse {
        sedang_dipanggil: vec![
            AntrianItem {
                no_antrian: "017".into(),
                nama_poli: "Poli Umum".into(),
                nama_pasien: "BUDI SANTOSO".into(),
                status: "DIPANGGIL".into(),
                ts_panggilan: "".into(),
                no_rawat: "".into(),
                kd_dokter: "".into(),
                kd_poli: "".into(),
            },
            AntrianItem {
                no_antrian: "005".into(),
                nama_poli: "Poli Gigi".into(),
                nama_pasien: "SITI RAHAYU".into(),
                status: "DIPANGGIL".into(),
                ts_panggilan: "".into(),
                no_rawat: "".into(),
                kd_dokter: "".into(),
                kd_poli: "".into(),
            },
        ],
        menunggu: vec![
            AntrianItem {
                no_antrian: "018".into(),
                nama_poli: "Poli Umum".into(),
                nama_pasien: "AHMAD FAUZI".into(),
                status: "MENUNGGU".into(),
                ts_panggilan: "".into(),
                no_rawat: "".into(),
                kd_dokter: "".into(),
                kd_poli: "".into(),
            },
            AntrianItem {
                no_antrian: "006".into(),
                nama_poli: "Poli Gigi".into(),
                nama_pasien: "DEWI LESTARI".into(),
                status: "MENUNGGU".into(),
                ts_panggilan: "".into(),
                no_rawat: "".into(),
                kd_dokter: "".into(),
                kd_poli: "".into(),
            },
            AntrianItem {
                no_antrian: "003".into(),
                nama_poli: "Poli Anak".into(),
                nama_pasien: "RIZKY FIRMANSYAH".into(),
                status: "MENUNGGU".into(),
                ts_panggilan: "".into(),
                no_rawat: "".into(),
                kd_dokter: "".into(),
                kd_poli: "".into(),
            },
            AntrianItem {
                no_antrian: "019".into(),
                nama_poli: "Poli Umum".into(),
                nama_pasien: "MARIA ULFAH".into(),
                status: "MENUNGGU".into(),
                ts_panggilan: "".into(),
                no_rawat: "".into(),
                kd_dokter: "".into(),
                kd_poli: "".into(),
            },
            AntrianItem {
                no_antrian: "002".into(),
                nama_poli: "Poli Dalam".into(),
                nama_pasien: "HENDRA WIJAYA".into(),
                status: "MENUNGGU".into(),
                ts_panggilan: "".into(),
                no_rawat: "".into(),
                kd_dokter: "".into(),
                kd_poli: "".into(),
            },
        ],
        jam: now.format("%H:%M:%S").to_string(),
        call_seq: 1,
        tanggal: format_tanggal_indo(now),
        last_called_no_antrian: None,
    })
}

async fn get_poli(
    State(state): State<Arc<AppState>>,
) -> Result<Json<Vec<PoliItem>>, (StatusCode, String)> {
    let rows = sqlx::query_as::<_, PoliItem>(
        "SELECT kd_poli, nm_poli FROM poliklinik WHERE status='1' ORDER BY nm_poli",
    )
    .fetch_all(&state.db)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(rows))
}

async fn get_list_pasien(
    State(state): State<Arc<AppState>>,
    Query(params): Query<ListPasienQuery>,
) -> Result<Json<Vec<AntrianItem>>, (StatusCode, String)> {
    let rows = sqlx::query_as::<_, AntrianItem>(
        r#"
        SELECT
            rp.no_reg AS no_antrian,
            po.nm_poli AS nama_poli,
            ps.nm_pasien AS nama_pasien,
            IF(ap.no_rawat IS NOT NULL, 'DIPANGGIL', 'MENUNGGU') AS status,
            COALESCE(mb.diterima, '0000-00-00 00:00:00') as ts_panggilan,
            rp.no_rawat,
            rp.kd_dokter,
            rp.kd_poli
        FROM reg_periksa rp
        JOIN poliklinik po ON rp.kd_poli = po.kd_poli
        JOIN pasien ps ON rp.no_rkm_medis = ps.no_rkm_medis
        LEFT JOIN antripoli ap ON rp.no_rawat = ap.no_rawat
        LEFT JOIN mutasi_berkas mb ON rp.no_rawat = mb.no_rawat
        WHERE rp.kd_poli = ? 
          AND rp.tgl_registrasi = CURDATE()
          AND rp.stts NOT IN ('Batal', 'Sudah')
        ORDER BY rp.no_reg ASC
        "#,
    )
    .bind(params.kd_poli)
    .fetch_all(&state.db)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(rows))
}

async fn panggil_loket(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<PanggilRequest>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    // 1. Delete existing for same doctor+poli (like Khanza)
    sqlx::query("DELETE FROM antripoli WHERE kd_dokter = ? AND kd_poli = ?")
        .bind(&payload.kd_dokter)
        .bind(&payload.kd_poli)
        .execute(&state.db)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    // 2. Insert new call
    sqlx::query(
        "INSERT INTO antripoli (kd_dokter, kd_poli, status, no_rawat) VALUES (?, ?, '1', ?)",
    )
    .bind(&payload.kd_dokter)
    .bind(&payload.kd_poli)
    .bind(&payload.no_rawat)
    .execute(&state.db)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    // 3. Update mutasi_berkas (to trigger the call_seq update logic)
    let exists: bool =
        sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM mutasi_berkas WHERE no_rawat = ?)")
            .bind(&payload.no_rawat)
            .fetch_one(&state.db)
            .await
            .unwrap_or(false);

    if exists {
        sqlx::query(
            "UPDATE mutasi_berkas SET status='Sudah Diterima', diterima=NOW() WHERE no_rawat = ?",
        )
        .bind(&payload.no_rawat)
        .execute(&state.db)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    } else {
        sqlx::query("INSERT INTO mutasi_berkas (no_rawat, status, tgl_muncul, jam_muncul, diterima) VALUES (?, 'Sudah Diterima', CURDATE(), CURTIME(), NOW())")
            .bind(&payload.no_rawat)
            .execute(&state.db)
            .await
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    }

    // 4. Increment call_seq manually to ensure frontend detects the call
    state.call_seq.fetch_add(1, Ordering::SeqCst);

    Ok(Json(json!({"status": "success"})))
}

async fn get_jadwal(
    State(state): State<Arc<AppState>>,
) -> Result<Json<Vec<JadwalItem>>, (StatusCode, String)> {
    let rows = sqlx::query_as::<_, JadwalItem>(
        r#"
        SELECT 
            d.nm_dokter, 
            p.nm_poli, 
            CAST(j.jam_mulai AS CHAR) as jam_mulai, 
            CAST(j.jam_selesai AS CHAR) as jam_selesai
        FROM jadwal j
        JOIN dokter d ON j.kd_dokter = d.kd_dokter
        JOIN poliklinik p ON j.kd_poli = p.kd_poli
        WHERE j.hari_kerja = (
          CASE DAYNAME(CURDATE())
            WHEN 'Sunday'    THEN 'AKHAD'
            WHEN 'Monday'    THEN 'SENIN'
            WHEN 'Tuesday'   THEN 'SELASA'
            WHEN 'Wednesday' THEN 'RABU'
            WHEN 'Thursday'  THEN 'KAMIS'
            WHEN 'Friday'    THEN 'JUMAT'
            WHEN 'Saturday'  THEN 'SABTU'
            ELSE ''
          END
        )
        ORDER BY j.jam_mulai ASC
        "#,
    )
    .fetch_all(&state.db)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(rows))
}

// ─── Auto-Clear Task ──────────────────────────────────────────────────────────

/// Hapus semua baris antripoli yang no_rawat-nya BUKAN hari ini.
/// Dipanggil saat startup dan otomatis tiap pergantian hari.
async fn hapus_antripoli_lama(pool: &MySqlPool) {
    let today = chrono::Local::now().format("%Y/%m/%d").to_string();
    let pola = format!("{}/%", today);

    match sqlx::query("DELETE FROM antripoli WHERE no_rawat NOT LIKE ? AND no_rawat != ''")
        .bind(&pola)
        .execute(pool)
        .await
    {
        Ok(r) => {
            if r.rows_affected() > 0 {
                tracing::info!(
                    "🧹 Auto-clear: {} baris antripoli lama dihapus (bukan {})",
                    r.rows_affected(),
                    today
                );
            }
        }
        Err(e) => tracing::error!("❌ Gagal hapus antripoli lama: {}", e),
    }
}

/// Background task: setiap menit cek apakah tanggal sudah berganti,
/// lalu hapus antripoli lama.
async fn auto_clear_loop(pool: MySqlPool) {
    let mut last_date = chrono::Local::now().format("%Y/%m/%d").to_string();

    loop {
        tokio::time::sleep(tokio::time::Duration::from_secs(60)).await;

        let today = chrono::Local::now().format("%Y/%m/%d").to_string();
        if today != last_date {
            tracing::info!(
                "📅 Tanggal berganti → {} | Membersihkan antripoli lama…",
                today
            );
            hapus_antripoli_lama(&pool).await;
            last_date = today;
        }
    }
}

// ─── Main ─────────────────────────────────────────────────────────────────────

#[tokio::main]
async fn main() {
    // Load .env jika ada
    dotenvy::dotenv().ok();

    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::new(
            std::env::var("RUST_LOG").unwrap_or_else(|_| "info".into()),
        ))
        .with(tracing_subscriber::fmt::layer())
        .init();

    // Mode demo (tanpa DB) atau production (dengan DB)
    let use_demo = std::env::var("DEMO_MODE").unwrap_or_else(|_| "true".into()) == "true";

    let app = if use_demo {
        tracing::info!("🟡 Berjalan dalam mode DEMO (tanpa database)");
        Router::new()
            .route("/", get(index))
            .route("/api/antrian", get(get_antrian_demo))
            .nest_service("/static", ServeDir::new("static"))
            .layer(CorsLayer::permissive())
    } else {
        let database_url = std::env::var("DATABASE_URL")
            .expect("DATABASE_URL harus diset di .env untuk mode production");

        let pool = MySqlPool::connect(&database_url)
            .await
            .expect("Gagal koneksi ke database");

        tracing::info!("🟢 Koneksi database berhasil");

        // Bersihkan antripoli lama saat startup
        hapus_antripoli_lama(&pool).await;

        // Jalankan background task ganti hari
        tokio::spawn(auto_clear_loop(pool.clone()));

        let state = Arc::new(AppState {
            db: pool,
            call_seq: AtomicU64::new(0),
            last_table_ts: tokio::sync::Mutex::new(String::new()),
        });

        Router::new()
            .route("/", get(index))
            .route("/api/antrian", get(get_antrian))
            .route("/api/poli", get(get_poli))
            .route("/api/list-pasien", get(get_list_pasien))
            .route("/api/panggil-loket", post(panggil_loket))
            .route("/api/jadwal", get(get_jadwal))
            .nest_service("/static", ServeDir::new("static"))
            .layer(CorsLayer::permissive())
            .with_state(state)
    };

    let port = std::env::var("PORT").unwrap_or_else(|_| "3030".into());
    let addr = format!("0.0.0.0:{}", port);

    let listener = match tokio::net::TcpListener::bind(&addr).await {
        Ok(l) => l,
        Err(e) => {
            eprintln!("\n❌ Gagal bind ke port {} — {}", port, e);
            eprintln!("   Port sudah dipakai proses lain.");
            eprintln!("   Jalankan perintah ini untuk membebaskan port:");
            eprintln!("   lsof -ti:{} | xargs kill -9\n", port);
            std::process::exit(1);
        }
    };

    tracing::info!("🚀 Server berjalan di http://{}", addr);
    tracing::info!("📺 Buka browser: http://localhost:{}", port);

    if let Err(e) = axum::serve(listener, app).await {
        eprintln!("❌ Server error: {}", e);
        std::process::exit(1);
    }
}
