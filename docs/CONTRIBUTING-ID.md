# Berkontribusi dalam pengembangan RustDesk

RustDesk mengajak semua orang untuk ikut berkontribusi. Berikut ini adalah panduan jika kamu sedang mempertimbangkan untuk memberikan bantuan kepada kami:

## Kontirbusi

Untuk melakukan kontribusi pada RustDesk atau dependensinya, sebaiknya dilakukan dalam bentuk pull request di GitHub. Setiap permintaan pull request akan ditinjau oleh kontributor utama atau seseorang yang memiliki wewenang untuk menggabungkan perubahan kode, baik yang sudah dimasukkan ke dalam struktur utama ataupun memberikan umpan balik untuk perubahan yang akan diperlukan. Setiap kontribusi harus sesuai dengan format ini, juga termasuk yang berasal dari kontributor utama.

Apabila kamu ingin mengatasi sebuah masalah yang sudah ada di daftar issue, harap klaim terlebih dahulu dengan memberikan komentar pada GitHub issue yang ingin kamu kerjakan. Hal ini dilakukan untuk mencegah terjadinya duplikasi dari kontributor pada daftar issue yang sama.

## Pemeriksaan Pull Request

- Branch yang menjadi acuan adalah branch master dari repositori utama dan, jika diperlukan, lakukan rebase ke branch  master yang terbaru sebelum kamu mengirim pull request. Apabila terdapat masalah kita melakukan proses merge ke branch master kemungkinan kamu akan diminta untuk melakukan rebase pada perubahan yang sudah dibuat.

- Sebaiknya buatlah commit seminimal mungkin, sambil memastikan bahwa setiap commit yang dibuat sudah benar (contohnya, setiap commit harus bisa di kompilasi dan berhasil melewati tahap test).

- Setiap commit harus disertai dengan tanda tangan Sertifikat Asal Pengembang (Developer Certificate of Origin) (<http://developercertificate.org>), yang mengindikasikan bahwa kamu (and your employer if applicable) bersedia untuk patuh terhadap persyaratan dari [lisensi projek](../LICENCE). Di git bash, ini adalah opsi parameter `-s` pada `git commit`

- Jika perubahan yang kamu buat tidak mendapat tinjauan atau kamu membutuhkan orang tertentu untuk meninjaunya, kamu bisa @-reply seorang reviewer meminta peninjauan dalam permintaan pull request atau komentar, atau kamu bisa meminta tinjauan melalui [email](mailto:info@rustdesk.com).

- Sertakan test yang relevan terhadap bug atau fitur baru yang sudah dikerjakan.

Untuk instruksi Git yang lebih lanjut, cek disini [GitHub workflow 101](https://github.com/servo/servo/wiki/GitHub-workflow).

## Tindakan

<https://github.com/rustdesk/rustdesk/blob/master/docs/CODE_OF_CONDUCT-ID.md>

## Komunikasi

Kontributor RustDesk sering berkunjung ke [Discord](https://discord.gg/nDceKgxnkV).
