# Pemeriksaan forex dan komoditas — 2 November 2026

Domain publik tetap **https://api.dollarkilat.xyz**. Perbaikan tersedia dalam
source proyek; server produksi belum di-deploy dari pekerjaan ini.

## Bukti dan penyebab

- Produksi menjawab HTTP 200 saat diperiksa. Board mengirim `code`/`prev_rate`
  tanpa seluruh field `ForexQuote` desktop, sehingga deserialisasi gagal.
- Produksi mengirim `change_pct: 0.15` untuk seluruh simbol yang diperiksa.
  Implementasi repo juga berisi harga komoditas tetap dan fallback angka contoh.
- History server memakai `points`, sedangkan desktop membaca `closes`.
- Header Cache-Control respons saja tidak menjalankan cache upstream Worker.
- Tidak ada bukti HTTP 429 dari pemeriksaan produksi ini; batas penyedia tetap
  ditangani sebagai kemungkinan kegagalan, bukan dianggap satu-satunya penyebab.

## Perbaikan

- Board sesuai kontrak desktop; alias lama tetap tersedia. History mengirim
  kedua format, dengan konversi hari Unix dan satuan harga yang sesuai.
- Harga komoditas berasal dari penyedia. Data yang tidak tersedia tidak diisi
  angka tetap. Kurs/logam dapat memakai currency-api harian sebagai cadangan.
- Cache per sumber/simbol, deduplikasi permintaan bersamaan, timeout, jeda
  kegagalan, dan penghormatan Retry-After. Data lama bertanda stale mempertahankan
  waktu sumber; masa simpan terakhir maksimal 24 jam.
- Sinkronisasi kelima varian deploy dari satu forex-source.js, beserta pemeriksaan
  sinkronisasi yang dapat diulang. Lihat workers/dollarkilat-api/README.md.
- Desktop menghindari cache HTTP Windows, memakai timeout hub yang cukup,
  memperbarui kurs IDR untuk history, dan mempertahankan board terakhir saat semua
  sumber gagal. Cadangan hanya mengirim simbol yang benar-benar punya data;
  pergerakan, rentang, dan sparkline contoh tidak dikirim sebagai pengamatan.
- VND konsisten per 1000; futures pertanian dalam sen dikonversi ke dolar,
  termasuk history dan tampilan harga/perubahan di desktop.

## Validasi

- Node: 35/35 tes lulus pada Worker root, Pages root, paket Worker, paket Pages,
  dan Pages Functions. Pengujian memakai respons penyedia terkendali, mencakup
  schema, riwayat, outage, 429, cache, kedaluwarsa, pemulihan, dan konversi.
- `sync-forex.mjs --check` dan `git diff --check` lulus.
- `cargo test --workspace --locked --offline --no-fail-fast`: 1060 lulus,
  0 gagal, 6 ignored; doc tests lulus. Log lokal: market-test-results.log.
- Eksekusi awal dalam lingkungan terbatas gagal pada
  `services::http::cache_tests::a_repeated_get_without_cache_headers_comes_from_windows_cache`
  dan empat tes pairdrop: `a_cut_send_keeps_nothing`,
  `a_phone_sends_files_and_saves_one_over_a_direct_link`,
  `hidden_phone_addresses_still_connect`,
  `pairing_then_sending_through_the_meeting_server`. Semuanya lulus pada tes akhir
  dengan akses cache/sertifikat Windows normal.
- Review kode memperbaiki metadata history logam agar usia/stale kurs konversi
  ikut tercatat. `cargo check --workspace --locked --offline` lulus.
- Gate lint ketat Rust masih gagal pada lima lint lama: `manual_is_multiple_of`
  di forex.rs, health.rs, update.rs; `result_unit_err` di phonedrop/http.rs;
  `too_many_arguments` di us_stocks.rs. Tidak ada perubahan pada baris-baris itu.
  Karena itu pemeriksaan Rust belum dapat disebut bersih dari lint.

## Batas pemeriksaan dan biaya

Pemeriksaan mencakup alur data terkait, varian deploy, review perubahan, dan tes
seluruh workspace; bukan audit manual setiap baris atau pengujian visual seluruh
fitur. Penyedia gratis bisa tertunda atau tidak menyediakan suatu instrumen.
Harga GOLD memakai estimasi premium, bukan harga jual resmi Antam. Riwayat logam
memakai kurs IDR saat ini, sesuai pendekatan desktop sebelumnya.

Tidak ada crate, dependensi runtime, thread, timer, atau polling baru. Cache
memori Worker dibatasi 128 entri dan cache edge terpisah per lokasi; bukan kuota
global. Aturan aplikasi berhenti saat terkunci/display mati/battery saver tetap
berlaku. Ukuran installer dan memori idle belum diukur; tidak ada rilis dibuat.

Untuk produksi, deploy source Worker/Pages yang diperbarui lalu periksa board
dan history pada domain tetap. Zip deploy lama belum diperbarui; gunakan folder
deploy-pages yang sudah disinkronkan, bukan zip lama.
