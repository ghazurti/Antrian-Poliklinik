// ─── DOM Refs ─────────────────────────────────────────────────────────────────
const elPoli = document.getElementById('poli-select');
const elBody = document.getElementById('patient-body');
const elLoading = document.getElementById('loading');
const btnRefresh = document.getElementById('btn-refresh');

// ─── Init ─────────────────────────────────────────────────────────────────────

async function init() {
    await fetchPoli();

    elPoli.addEventListener('change', () => fetchPatients());
    btnRefresh.addEventListener('click', () => fetchPatients());
}

// ─── Fetch Poli ───────────────────────────────────────────────────────────────

async function fetchPoli() {
    try {
        const res = await fetch('/api/poli');
        const data = await res.json();

        elPoli.innerHTML = '<option value="">-- Pilih Poli --</option>' +
            data.map(p => `<option value="${p.kd_poli}">${p.nm_poli}</option>`).join('');
    } catch (err) {
        console.error('Gagal fetch poli:', err);
    }
}

// ─── Fetch Patients ───────────────────────────────────────────────────────────

async function fetchPatients() {
    const kdPoli = elPoli.value;
    if (!kdPoli) {
        elBody.innerHTML = '<tr><td colspan="4" style="text-align: center; color: var(--text-soft); padding: 2rem;">Pilih poliklinik terlebih dahulu</td></tr>';
        return;
    }

    elLoading.classList.remove('hidden');

    try {
        const res = await fetch(`/api/list-pasien?kd_poli=${kdPoli}`);
        const data = await res.json();

        renderPatients(data);
    } catch (err) {
        console.error('Gagal fetch pasien:', err);
        elBody.innerHTML = '<tr><td colspan="4" style="text-align: center; color: #ef4444; padding: 2rem;">Gagal memuat data pasien</td></tr>';
    } finally {
        elLoading.classList.add('hidden');
    }
}

// ─── Render ───────────────────────────────────────────────────────────────────

function renderPatients(patients) {
    if (patients.length === 0) {
        elBody.innerHTML = '<tr><td colspan="4" style="text-align: center; color: var(--text-soft); padding: 2rem;">Tidak ada pasien terdaftar hari ini</td></tr>';
        return;
    }

    elBody.innerHTML = patients.map(p => {
        const isDipanggil = p.status === 'DIPANGGIL';
        return `
      <tr>
        <td style="font-weight: 800; color: var(--accent);">${p.no_antrian}</td>
        <td>
          <div style="font-weight: 600;">${p.nama_pasien}</div>
          <div style="font-size: 0.75rem; color: var(--text-soft);">${p.no_rawat}</div>
        </td>
        <td>
          <span class="badge badge-${p.status.toLowerCase()}">${p.status}</span>
        </td>
        <td>
          <button class="btn btn-panggil" onclick="panggil('${p.no_rawat}', '${p.kd_dokter}', '${p.kd_poli}')">
            ${isDipanggil ? 'Panggil Ulang' : 'Panggil'}
          </button>
        </td>
      </tr>
    `;
    }).join('');
}

// ─── Action ───────────────────────────────────────────────────────────────────

async function panggil(noRawat, kdDokter, kdPoli) {
    try {
        const res = await fetch('/api/panggil-loket', {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({ no_rawat: noRawat, kd_dokter: kdDokter, kd_poli: kdPoli })
        });

        if (res.ok) {
            // Refresh list setelah panggil
            fetchPatients();
        } else {
            alert('Gagal melakukan panggilan');
        }
    } catch (err) {
        console.error('Gagal panggil:', err);
        alert('Error koneksi ke server');
    }
}

init();
