/**
 * app.js — Logika frontend Antrian Poli (TV Display)
 */

// ─── State ───────────────────────────────────────────────────────────────────

let lastCallSeq = -1;
let callQueue = [];
let isProcessingQueue = false;
let lastJadwalJson = ""; // Untuk deteksi perubahan data jadwal

// ─── DOM Refs ─────────────────────────────────────────────────────────────────

const elJam = document.getElementById('jam');
const elTanggal = document.getElementById('tanggal');
const elCalled = document.getElementById('called-list');
const elTicker = document.getElementById('ticker-text');
const elOverlay = document.getElementById('call-overlay');
const elOverlayNumber = document.getElementById('overlay-number');
const elOverlayName = document.getElementById('overlay-name');
const elOverlayPoli = document.getElementById('overlay-poli');
const elSchedule = document.getElementById('schedule-list');

// ─── Jam Real-time ──────────────────────────────────────────────────────────

function updateClock() {
  const now = new Date();
  const hh = String(now.getHours()).padStart(2, '0');
  const mm = String(now.getMinutes()).padStart(2, '0');
  const ss = String(now.getSeconds()).padStart(2, '0');
  elJam.textContent = `${hh}:${mm}:${ss}`;
}
setInterval(updateClock, 1000);
updateClock();

// ─── Polling & Data ─────────────────────────────────────────────────────────

async function fetchAntrian() {
  try {
    const res = await fetch('/api/antrian');
    if (!res.ok) throw new Error(`HTTP ${res.status}`);

    const data = await res.json();
    if (data.tanggal) elTanggal.textContent = data.tanggal;

    // Deteksi perubahan lewat call_seq (termasuk panggil ulang)
    const seq = data.call_seq ?? 0;
    if (seq > lastCallSeq && lastCallSeq >= 0) {
      const targetNo = data.last_called_no_antrian;
      console.log('🔔 Panggilan terdeteksi:', targetNo, 'Seq:', seq);

      const targetPatient = data.sedang_dipanggil.find(p => String(p.no_antrian).trim() === String(targetNo).trim());

      if (targetPatient) {
        console.log('🚀 Memasukkan ke antrean suara:', targetPatient.nama_pasien);
        callQueue.push(targetPatient);
        if (!isProcessingQueue) processCallQueue();
      } else {
        console.warn('⚠️ Pasien tidak ditemukan di daftar sedang_dipanggil untuk nomor:', targetNo);
      }
    }
    lastCallSeq = seq;

    renderCalled(data.sedang_dipanggil);
    updateTicker(data.sedang_dipanggil);

  } catch (err) {
    console.error('Gagal fetch antrian:', err);
  }
}

// ─── Fetch Jadwal ─────────────────────────────────────────────────────────────

async function fetchJadwal() {
  try {
    const res = await fetch('/api/jadwal');
    if (!res.ok) throw new Error(`HTTP ${res.status}`);
    const data = await res.json();

    // Hanya re-render jika data berubah agar tidak memutus auto-scroll
    const currentJson = JSON.stringify(data);
    if (currentJson !== lastJadwalJson) {
      renderJadwal(data);
      lastJadwalJson = currentJson;
    }
  } catch (err) {
    console.error('Gagal fetch jadwal:', err);
  }
}

function renderJadwal(items) {
  if (!items || items.length === 0) {
    elSchedule.innerHTML = '<tr><td colspan="3" class="empty-state">Tidak ada jadwal praktek hari ini</td></tr>';
    return;
  }
  elSchedule.innerHTML = items.map(item => `
    <tr>
      <td><div class="doc-name">${escHtml(item.nm_dokter)}</div></td>
      <td><div class="poli-name">${escHtml(item.nm_poli)}</div></td>
      <td><span class="jam-badge">${item.jam_mulai.substring(0, 5)} – ${item.jam_selesai.substring(0, 5)}</span></td>
    </tr>
  `).join('');
}

// ─── Auto Scroll Jadwal ───────────────────────────────────────────────────────

function initAutoScroll() {
  const container = document.querySelector('.schedule-table-wrap');
  if (!container) return;

  let pos = 0;
  const speed = 0.5; // Pixel per frame

  function step() {
    if (callQueue.length > 0 || isProcessingQueue) {
      // Pause scroll jika ada panggilan agar tidak mengganggu fokus
      requestAnimationFrame(step);
      return;
    }

    pos += speed;
    container.scrollTop = pos;

    // Jika sampai bawah, tunggu sebentar lalu balik ke atas
    if (pos >= (container.scrollHeight - container.clientHeight)) {
      setTimeout(() => {
        pos = 0;
        container.scrollTop = 0;
        requestAnimationFrame(step);
      }, 3000); // Tunggu 3 detik di bawah
    } else {
      requestAnimationFrame(step);
    }
  }

  // Tunggu 5 detik sebelum mulai scroll pertama kali
  setTimeout(() => requestAnimationFrame(step), 5000);
}

// ─── Render UI ───────────────────────────────────────────────────────────────

function renderCalled(items) {
  const countEl = document.getElementById('called-count');
  if (!items || items.length === 0) {
    elCalled.innerHTML = '<div class="empty-state">Belum ada antrian yang dipanggil</div>';
    if (countEl) countEl.textContent = '0';
    return;
  }
  if (countEl) countEl.textContent = items.length;
  elCalled.innerHTML = items.map(item => `
    <div class="called-card">
      <div class="called-number">${escHtml(item.no_antrian)}</div>
      <div class="called-info">
        <div class="called-poli">${escHtml(item.nama_poli)}</div>
        <div class="called-name">${escHtml(item.nama_pasien)}</div>
      </div>
      <div class="called-badge">
        <span class="called-badge-icon">🔔</span>
        <span class="called-badge-text">Dipanggil</span>
      </div>
    </div>
  `).join('');
}

// ─── Antrean Panggilan (Visual & Suara) ──────────────────────────────────────

async function processCallQueue() {
  if (callQueue.length === 0) {
    isProcessingQueue = false;
    return;
  }

  isProcessingQueue = true;
  const patient = callQueue.shift();

  // 1. Tampilkan Visual Overlay
  elOverlayNumber.textContent = patient.no_antrian;
  elOverlayName.textContent = patient.nama_pasien;
  elOverlayPoli.textContent = patient.nama_poli;
  elOverlay.classList.remove('hidden');

  // 2. Bunyikan Bel (Ding)
  playDing();

  // 3. Suara TTS (Tunggu ding selesai ~1 detik)
  await new Promise(r => setTimeout(r, 1000));
  await speakPanggilanSync(patient);

  // 4. Tunggu visual tampil sebentar setelah suara habis
  await new Promise(r => setTimeout(r, 2000));
  elOverlay.classList.add('hidden');

  // Jeda antar panggilan
  await new Promise(r => setTimeout(r, 1500));

  processCallQueue();
}

// ─── Audio clip player ───────────────────────────────────────────────────────

function playClip(name) {
  return new Promise((resolve) => {
    const audio = new Audio(`/static/audio/${name}.mp3`);
    audio.onended = resolve;
    audio.onerror = resolve; // fallback: lanjut walau file tidak ada
    audio.play().catch(resolve);
  });
}

function pause(ms) {
  return new Promise(r => setTimeout(r, ms));
}

// Pecah nomor antrian jadi digit, contoh "017" → ["0","1","7"]
function digitList(noAntrian) {
  return String(noAntrian).replace(/\D/g, '').split('');
}

function ttsSpeakSync(text, { rate = 0.88, pitch = 1.0 } = {}) {
  return new Promise((resolve) => {
    if (!window.speechSynthesis) return resolve();
    window.speechSynthesis.cancel();
    const utter = new SpeechSynthesisUtterance(text);
    const voices = window.speechSynthesis.getVoices();
    const idVoice = voices.find(v => v.lang.startsWith('id')) ||
                    voices.find(v => v.lang.includes('ID'));
    if (idVoice) utter.voice = idVoice;
    utter.lang = 'id-ID';
    utter.rate = rate;
    utter.pitch = pitch;
    utter.volume = 1.0;
    utter.onend = resolve;
    utter.onerror = resolve;
    window.speechSynthesis.speak(utter);
  });
}

async function speakPanggilanSync(item) {
  const nama = item.nama_pasien
    .toLowerCase()
    .replace(/\b\w/g, c => c.toUpperCase());

  const poli = item.nama_poli
    .replace(/Poliklinik\s*/gi, 'Poli ')
    .replace(/dr\.\s*/gi, 'dokter ')
    .replace(/Sp\.\w+/gi, '')
    .split(' - ')[0].trim();

  // 1. "Mohon perhatian"
  await playClip('mohon_perhatian');
  await pause(350);

  // 2. "Nomor"
  await playClip('nomor');
  await pause(200);

  // 3. Digit-digit nomor antrian
  for (const d of digitList(item.no_antrian)) {
    await playClip(d);
    await pause(150);
  }
  await pause(300);

  // 4. Nama pasien — TTS
  await ttsSpeakSync(nama, { rate: 0.82 });
  await pause(350);

  // 5. "silakan menuju"
  await playClip('silakan_menuju');
  await pause(200);

  // 6. Nama poli — TTS
  await ttsSpeakSync(poli, { rate: 0.88 });

  console.log('🔈 Panggilan selesai:', nama);
}

// ─── Ticker ───────────────────────────────────────────────────────────────────

const tickerMessages = [
  'Selamat datang di layanan antrian poli. Harap mempersiapkan kartu identitas dan kartu pasien Anda.',
  'Mohon untuk tidak berpindah tempat saat menunggu panggilan antrian.',
  'Jika nomor antrian Anda dipanggil 3x dan tidak hadir, antrian akan dianggap hangus.',
];
let tickerIdx = 0;

function updateTicker(called = []) {
  const dipanggilTeks = called.length > 0
    ? `🔊 PANGGILAN: ${called.map(c => `${c.no_antrian} – ${c.nama_pasien}`).join(' | ')} ✦ `
    : '';
  const staticMsg = tickerMessages[tickerIdx % tickerMessages.length];
  tickerIdx++;
  elTicker.textContent = `${dipanggilTeks}${staticMsg}`;
}

// ─── Audio Helpers ────────────────────────────────────────────────────────────

let audioCtx = null;
function getAudioCtx() {
  if (!audioCtx) audioCtx = new (window.AudioContext || window.webkitAudioContext)();
  return audioCtx;
}

function playDing() {
  try {
    const ctx = getAudioCtx();
    // Melodi ala bandara (G major triad: G4, B4, D5)
    const freqs = [392.00, 493.88, 587.33];
    let time = ctx.currentTime;
    freqs.forEach((freq, i) => {
      const osc = ctx.createOscillator();
      const gain = ctx.createGain();
      osc.connect(gain);
      gain.connect(ctx.destination);
      osc.type = 'sine';
      osc.frequency.setValueAtTime(freq, time + i * 0.4); // Jeda lebih lama
      gain.gain.setValueAtTime(0, time + i * 0.4);
      gain.gain.linearRampToValueAtTime(0.4, time + i * 0.4 + 0.1); // Fade in halus
      gain.gain.exponentialRampToValueAtTime(0.001, time + i * 0.4 + 1.2); // Fade out panjang
      osc.start(time + i * 0.4);
      osc.stop(time + i * 0.4 + 1.2);
    });
  } catch (e) { console.warn('Audio error:', e); }
}

function escHtml(str) {
  return String(str).replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;');
}

// ─── Init ─────────────────────────────────────────────────────────────────────

// Unlock audio pada interaksi pertama
document.addEventListener('click', () => {
  try { getAudioCtx().resume(); } catch (e) { }
}, { once: true });

fetchAntrian();
fetchJadwal();
initAutoScroll();

setInterval(fetchAntrian, 2000);
setInterval(fetchJadwal, 300000); // 5 menit
