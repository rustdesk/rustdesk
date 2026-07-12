Docker konteynerinde devcontainer'ın başlatılmasından sonra, hata ayıklama modunda bir Linux ikili dosyası oluşturulur.

Şu anda devcontainer, hata ayıklama ve sürüm modunda hem Linux hem de Android derlemeleri sunmaktadır.

Aşağıda, belirli derlemeler oluşturmak için projenin kökünden çalıştırılması gereken komutlar yer almaktadır.

Komut | Derleme Türü | Mod
-|-|-
`.devcontainer/build.sh --debug linux` | Linux | hata ayıklama
`.devcontainer/build.sh --release linux` | Linux | sürüm
`.devcontainer/build.sh --debug android` | Android-arm64 | hata ayıklama
`.devcontainer/build.sh --release android` | Android-arm64 | sürüm
