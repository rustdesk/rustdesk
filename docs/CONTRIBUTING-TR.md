# RustDesk'a Katkı Sağlamak

RustDesk, herkesten katkıyı memnuniyetle karşılar. Eğer bize yardımcı olmayı düşünüyorsanız, işte rehberlik eden kurallar:

## Katkılar

RustDesk veya bağımlılıklarına yapılan katkılar, GitHub pull istekleri şeklinde yapılmalıdır. Her bir pull isteği, çekirdek katkıcı tarafından gözden geçirilecek (yamaları kabul etme izni olan biri) ve ana ağaca kabul edilecek veya gerekli değişiklikler için geri bildirim verilecektir. Tüm katkılar bu formata uymalıdır, çekirdek katkıcılardan gelenler bile.

Eğer bir konu üzerinde çalışmak isterseniz, önce üzerinde çalışmak istediğinizi belirten bir yorum yaparak konuyu talep ediniz. Bu, katkı sağlayanların aynı konuda çift çalışmasını engellemek içindir.

## Pull İstek Kontrol Listesi

- Master dalından dallandırın ve gerekiyorsa pull isteğinizi göndermeden önce mevcut master dalına rebase yapın. Eğer master ile temiz bir şekilde birleşmezse, değişikliklerinizi rebase yapmanız istenebilir.

- Her bir commit mümkün olduğunca küçük olmalıdır, ancak her commit'in bağımsız olarak doğru olduğundan emin olun (örneğin, her commit derlenebilir ve testleri geçmelidir).

- Commit'ler, bir Geliştirici Sertifikası ile desteklenmelidir (http://developercertificate.org). Bu, [proje lisansının](../LICENCE) koşullarına uymayı kabul ettiğinizi gösteren bir onaydır. Git'te bunu `git commit` seçeneği olarak `-s` seçeneği ile yapabilirsiniz.

- Yamalarınız gözden geçirilmiyorsa veya belirli bir kişinin gözden geçirmesine ihtiyacınız varsa, çekme isteği veya yorum içinde bir gözden geçirmeyi istemek için bir inceleyiciyi @etiketleyebilir veya inceleme için [e-posta](mailto:info@rustdesk.com) ile talep edebilirsiniz.

- Düzelttiğiniz hatanın veya eklediğiniz yeni özelliğin ilgili testlerini ekleyin.

Daha spesifik git talimatları için, [GitHub iş akışı 101](https://github.com/servo/servo/wiki/GitHub-workflow)'e bakınız.

## Davranış

https://github.com/rustdesk/rustdesk/blob/master/docs/CODE_OF_CONDUCT-TR.md

## İletişim

RustDesk katkı sağlayıcıları, [Discord](https://discord.gg/nDceKgxnkV) kanalını sık sık ziyaret ederler.
