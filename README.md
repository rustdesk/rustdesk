
<div align="center">

<h1>Çekme Tabanlı Ses Dönüşümü Web Arayüzü</h1>
Kolay kullanımlı VITS tabanlı ses dönüşümü (ses değiştirme) çerçevesi<br><br>

[![madewithlove](https://img.shields.io/badge/made_with-%E2%9D%A4-red?style=for-the-badge&labelColor=orange
)](https://github.com/RVC-Project/Retrieval-based-Voice-Conversion-WebUI)

<img src="https://counter.seku.su/cmoe?name=rvc&theme=r34" /><br>

[![Open In Colab](https://img.shields.io/badge/Colab-F9AB00?style=for-the-badge&logo=googlecolab&color=525252)](https://colab.research.google.com/github/RVC-Project/Retrieval-based-Voice-Conversion-WebUI/blob/main/Retrieval_based_Voice_Conversion_WebUI.ipynb)
[![Lisans](https://img.shields.io/badge/LICENSE-MIT-green.svg?style=for-the-badge)](https://github.com/RVC-Project/Retrieval-based-Voice-Conversion-WebUI/blob/main/LICENSE)
[![Huggingface](https://img.shields.io/badge/🤗%20-Spaces-yellow.svg?style=for-the-badge)](https://huggingface.co/lj1995/VoiceConversionWebUI/tree/main/)

[![Discord](https://img.shields.io/badge/RVC%20Geliştiricileri-Discord-7289DA?style=for-the-badge&logo=discord&logoColor=white)](https://discord.gg/HcsmBBGyVk)

[**Güncelleme Günlüğü**](https://github.com/RVC-Project/Retrieval-based-Voice-Conversion-WebUI/blob/main/docs/Changelog_TR.md) | [**Sıkça Sorulan Sorular**](https://github.com/RVC-Project/Retrieval-based-Voice-Conversion-WebUI/wiki/S%C4%B1k%C3%A7a-Sorulan-Sorular) | [**AutoDL·5 Kuruşla AI Şarkıcısı Eğitme**](https://github.com/RVC-Project/Retrieval-based-Voice-Conversion-WebUI/wiki/Autodl%E2%80%A25-Kuru%C5%9Fla-AI-%C5%9Eark%C4%B1c%C4%B1s%C4%B1-E%C4%9Fitme) | [**Karşılaştırmalı Deney Kayıtları**](https://github.com/RVC-Project/Retrieval-based-Voice-Conversion-WebUI/wiki/Autodl%E2%80%A25-Kuru%C5%9Fla-AI-%C5%9Eark%C4%B1c%C4%B1s%C4%B1-E%C4%9Fitme) | [**Çevrimiçi Demo**](https://huggingface.co/spaces/Ricecake123/RVC-demo)

</div>

------

[**İngilizce**](./docs/README.en.md) | [**中文简体**](./README.md) | [**日本語**](./docs/README.ja.md) | [**한국어**](./docs/README.ko.md) ([**韓國語**](./docs/README.ko.han.md)) | [**Türkçe**](./docs/README.tr.md)


> Gerçek zamanlı ses dönüşümü RVC kullanılarak gerçekleştirilmiştir: [w-okada/voice-changer](https://github.com/w-okada/voice-changer)

> Temel model, telif hakkı endişesi olmaksızın yaklaşık 50 saatlik açık kaynaklı yüksek kaliteli VCTK eğitim seti ile eğitilmiştir. 

> RVCv3 modelini bekleyin, daha büyük parametreler, daha fazla veri, daha iyi sonuçlar, temel hızı korurken daha az eğitim verisi gerektirir.

## Tanıtım
Bu depo aşağıdaki özelliklere sahiptir:
+ Ses renk sızdırmasını önlemek için giriş kaynağı özelliklerini eğitim seti özellikleri ile değiştirme üzerine en iyi 1 öğeyi kullanma
+ Daha düşük kaliteli ekran kartlarında bile hızlı eğitim yapabilme
+ Az miktarda veri ile bile iyi sonuçlar elde edebilme (en azından 10 dakika düşük gürültülü ses verisi toplamanızı öneririz)
+ Model birleştirme ile ses rengini değiştirebilme (ckpt işleme sekmesindeki ckpt-merge seçeneği ile)
+ Kullanımı kolay web arayüzü
+ İnsan sesini hızla ayırmak için UVR5 modelini çağırabilme
+ İleri seviyede [Ses Yüksekliği Çıkarma Algoritması InterSpeech2023-RMVPE](#Referans-Projeler) ile sessiz dönüşüme son verme. En iyi sonuç (belirgin şekilde) sunar, ancak crepe_full'den daha hızlı ve daha az kaynak tüketir
+ Nvidia A ve I kart hızlandırma desteği

## Ortam Kurulumu
Aşağıdaki komutları Python sürümünün 3.8'den büyük olduğu bir ortamda çalıştırmanız gerekmektedir.  

(Windows/Linux)  
Önce ana bağımlılıkları pip ile kurun:
```bash
# Pytorch ve temel bağımlılıkları yükleyin, zaten yüklüyse atlayabilirsiniz
# Referans: https://pytorch.org/get-started/locally/
pip install torch torchvision torchaudio

# Eğer Windows işletim sistemi kullanıyorsanız ve Nvidia Ampere mimarisine (RTX30xx) sahipseniz, #21 numaralı işlemin deneyimine göre, pytorch'un doğru cuda sürümünü belirtmeniz

 gerekebilir.
#pip install torch torchvision torchaudio --index-url https://download.pytorch.org/whl/cu117
```

Bağımlılıkları poetry kullanarak kurmak isterseniz:
```bash
# Poetry bağımlılık yönetim aracını yükleyin, zaten yüklüyse atlayabilirsiniz
# Referans: https://python-poetry.org/docs/#installation
curl -sSL https://install.python-poetry.org | python3 -

# Poetry ile bağımlılıkları kurun
poetry install
```

Bağımlılıkları pip kullanarak kurmak isterseniz:
```bash
N kartı kullanıyorsanız:

pip install -r requirements.txt

A kartı/I kartı kullanıyorsanız:
pip install -r requirements-dml.txt

```

------
Mac kullanıcıları `run.sh` dosyasını kullanarak bağımlılıkları kurabilir:
```bash
sh ./run.sh
```

## Diğer Ön Model Hazırlıkları
RVC, çıkarım ve eğitim için bazı önceden eğitilmiş modellere ihtiyaç duyar.

Bu modelleri [Hugging Face alanımızdan](https://huggingface.co/lj1995/VoiceConversionWebUI/tree/main/) indirebilirsiniz.

Aşağıda, RVC'nin gerektirdiği ön model ve diğer dosyaların adlarını içeren bir liste bulunmaktadır:
```bash
hubert_base.pt

./pretrained 

./uvr5_weights

v2 sürümü modelini test etmek isterseniz, ek olarak indirmeniz gerekebilir

./pretrained_v2 

Eğer Windows kullanıyorsanız, muhtemelen bu dosyaya ihtiyacınız olacaktır. Ffmpeg ve ffprobe zaten kuruluysa bu adımı atlayabilirsiniz; Ubuntu/Debian kullanıcıları apt install ffmpeg komutunu kullanarak bu kütüphaneleri kurabilirler, Mac kullanıcıları ise brew install ffmpeg komutunu kullanarak kurabilirler (önceden Brew'ı kurmanız gerekebilir).

./ffmpeg

https://huggingface.co/lj1995/VoiceConversionWebUI/blob/main/ffmpeg.exe

./ffprobe

https://huggingface.co/lj1995/VoiceConversionWebUI/blob/main/ffprobe.exe

Eğer en son RMVPE insan sesi yüksekliği çıkarma algoritmasını kullanmak isterseniz, ses yüksekliği çıkarma modeli parametrelerini indirip RVC ana dizinine koymalısınız.

https://huggingface.co/lj1995/VoiceConversionWebUI/blob/main/rmvpe.pt

    A kartı/I kartı kullanıcıları için dml ortamında kullanmak üzere, aşağıdaki dosyayı indirin

    https://huggingface.co/lj1995/VoiceConversionWebUI/blob/main/rmvpe.onnx

```
Daha sonra aşağıdaki komutu kullanarak WebUI'yi başlatabilirsiniz:
```bash
python infer-web.py
```

Windows veya macOS kullanıyorsanız, `RVC-beta.7z` dosyasını indirebilir ve çıkarabilirsiniz. Windows kullanıcıları `go-web.bat` dosyasını çalıştırarak WebUI'yi başlatabilirler, macOS kullanıcıları ise `sh ./run.sh` komutunu kullanarak başlatabilirler.

Reponun içinde `Kolay Kullanım Kılavuzu.doc` adında bir belge de bulunmaktadır.

## Referans Projeler
+ [ContentVec](https://github.com/auspicious3000/contentvec/)
+ [VITS](https://github.com/jaywalnut310/vits)
+ [HIFIGAN](https://github.com/jik876/hifi-gan)
+ [Gradio](https://github.com/gradio-app/gradio)
+ [FFmpeg](https://github.com/FFmpeg/FFmpeg)
+ [Ultimate Vocal Remover](https://github.com/Anjok07/ultimatevocalremovergui)
+ [audio-slicer](https://github.com/openvpi/audio-slicer)
+ [Vocal pitch extraction: RMVPE](https://github.com/Dream-High/RMVPE)
  + Önceden eğitilmiş model [yxlllc](https://github.com/yxlllc/RMVPE) ve [RVC-Boss](https://github.com/RVC-Boss) tarafından eğitilmiş ve test edilmiştir.

## Katkı Sağlayan Tüm Kişilere Teşekkürler
<a href="https://github.com/RVC-Project/Retrieval-based-Voice-Conversion-WebUI/graphs/contributors" target="_blank">
  <img src="https://contrib.rocks/image?repo=RVC-Project/Retrieval-based-Voice-Conversion-WebUI" />
</a>
```

