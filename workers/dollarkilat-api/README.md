# DollarKilat Centralized Market Hub (api.dollarkilat.xyz)

## Perbaikan data forex dan komoditas

Domain publik tetap `https://api.dollarkilat.xyz`. Harga board berasal dari chart
Yahoo Finance; fallback kurs dan logam memakai currency-api (data harian, bukan
tick realtime). Penyedia gratis dapat menunda harga atau tidak menyediakan simbol.
Tidak ada harga tetap pengganti saat sumber gagal. `unavailable` berisi simbol
yang gagal, sedangkan `source`, `stale`, dan `updated_at` per quote menjelaskan
asal dan usia data. Logam dikonversi ke IDR/gram; GOLD memakai estimasi premium
1.035, bukan harga jual resmi Antam. JPY/KRW terhadap IDR dihitung per 100,
VND per 1000. Futures pertanian yang dikutip dalam sen AS dikonversi ke USD.

Kontrak desktop: `quotes[]` memuat `symbol`, `name`, `rate`, `open`, `prev_close`,
`low`, `high`, `change_pct`, `is_gold`, `unit`, `spark`. Alias lama `code` dan
`prev_rate` tetap tersedia. History memuat `points` (Unix seconds) serta `closes`
berupa pasangan `[hari Unix, harga]` sesuai desktop. Tidak ada riwayat sintetis.

Cache upstream 120 detik (history 10 menit, kurs harian 1 jam), berbagi lewat
Cloudflare Cache API dan cache memori terbatas 128 entri. HTTP 429 menghormati
`Retry-After`; kegagalan lain dijeda 60 detik. Cache harga terakhir bertahan
maksimal 24 jam dan ditandai stale tanpa mengganti timestamp. Tidak ada polling
baru atau dependensi runtime baru. Cache terpisah per edge Cloudflare, sehingga
bukan pembatas kuota global lintas lokasi.

Edit `forex-source.js`, lalu dari root repo jalankan:

```powershell
rtk proxy node workers/dollarkilat-api/sync-forex.mjs
rtk proxy node --test workers/dollarkilat-api/forex.test.mjs
```

Sinkronisasi mencakup Worker, Pages advanced mode, dan Pages Functions.
Setelah deploy, uji board dan history pada domain di atas. Paket zip lama tidak
otomatis berubah; gunakan `deploy-pages/` yang sudah diperbarui jika deploy Pages.
Perubahan source lokal belum berarti server produksi sudah diperbarui.

Cloudflare Worker serverless proxy & edge cache untuk **Oyenza Master**.
Menghubungkan dan menampung data dari:
- **US Stocks** (Quotes realtime, sparkline 5-menit, grafik 1D/1M/6M/1Y, serta pencarian autocomplete untuk **8.000+ saham & ETF AS**).
- **Forex & Commodities** (Kurs real-time, Spot Emas/Perak/Minyak/Gas Alam, serta katalog pencarian **160+ mata uang & komoditas dunia**).
- **Crypto** (Data harga Binance, 24h volume, dan pencarian koin).

Dilengkapi dengan **Cloudflare Edge Cache** otomatis untuk melindungi dari rate limit, IP ban, dan menjamin kecepatan respon < 20 ms.

---

## Cara Deploy ke Cloudflare (Pilih salah satu cara)

### Opsi A: Melalui Browser / Cloudflare Dashboard (Paling Mudah, Tanpa Install Apapun)

1. Buka [dash.cloudflare.com](https://dash.cloudflare.com/) dan login ke akun Cloudflare Anda.
2. Di menu samping kiri, klik **Workers & Pages** -> **Create application** -> **Create Worker**.
3. Beri nama worker Anda, misalnya: `dollarkilat-api`, lalu klik **Deploy**.
4. Setelah ter-deploy, klik tombol **Edit code**.
5. Hapus semua kode default di editor, lalu **copy-paste** isi file [`worker.js`](worker.js) ini ke sana.
6. Klik **Save and deploy** di kanan atas.
7. **Hubungkan ke Subdomain `api.dollarkilat.xyz`**:
   - Kembali ke halaman worker `dollarkilat-api`.
   - Buka tab **Settings** -> **Domains & Routes** -> klik **Add** -> pilih **Custom Domain**.
   - Masukkan: `api.dollarkilat.xyz` (pastikan domain `dollarkilat.xyz` sudah aktif di Cloudflare Anda).
   - Klik **Add Custom Domain**. Selesai! Subdomain Anda langsung aktif dengan SSL HTTPS gratis.

---

### Opsi B: Melalui Terminal / CLI (Wrangler)

1. Buka terminal di folder ini (`workers/dollarkilat-api`):
   ```bash
   cd workers/dollarkilat-api
   ```
2. Login ke akun Cloudflare Anda:
   ```bash
   npx wrangler login
   ```
3. Deploy worker:
   ```bash
   npx wrangler deploy
   ```
   Wrangler akan otomatis mendaftarkan worker dan menghubungkan rutenya ke `api.dollarkilat.xyz`.

---

## Pengujian Endpoint (Bisa dicoba di browser)

Setelah aktif, Anda bisa langsung membuka URL berikut di browser atau Postman:

- **Status Server**:
  `https://api.dollarkilat.xyz/health`
- **Quotes Saham AS (Realtime + Sparkline)**:
  `https://api.dollarkilat.xyz/v1/us-stocks/quotes?symbols=AAPL,NVDA,TSLA,MSFT`
- **Pencarian 8.000+ Saham AS**:
  `https://api.dollarkilat.xyz/v1/us-stocks/search?q=apple`
- **Grafik 1 Tahun Saham AS**:
  `https://api.dollarkilat.xyz/v1/us-stocks/history?symbol=AAPL&range=1y`
- **Forex & Komoditas (Emas, Perak, Minyak, Kurs IDR)**:
  `https://api.dollarkilat.xyz/v1/forex/board?symbols=USD/IDR,EUR/IDR,GOLD,SILVER,OIL`
- **Pencarian 160+ Mata Uang & Komoditas**:
  `https://api.dollarkilat.xyz/v1/forex/search?q=jpy`
- **Crypto Realtime (Binance)**:
  `https://api.dollarkilat.xyz/v1/crypto/board?symbols=BTC,ETH,SOL`
