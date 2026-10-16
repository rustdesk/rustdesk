// Translations for strings added by this fork. Kept out of src/lang/*.rs so upstream merges
// never conflict with them. Languages not listed here fall back to the English key.
const KEYS: [&str; 4] = [
    "Server profiles",
    "Profile name",
    "Save current",
    "Edit profile",
];

#[rustfmt::skip]
const TABLE: &[(&str, [&str; 4])] = &[
    ("hu", ["Szerverprofilok", "Profil neve", "Jelenlegi mentése", "Profil szerkesztése"]),
    ("de", ["Serverprofile", "Profilname", "Aktuelles speichern", "Profil bearbeiten"]),
    ("fr", ["Profils de serveur", "Nom du profil", "Enregistrer l'actuel", "Modifier le profil"]),
    ("es", ["Perfiles de servidor", "Nombre del perfil", "Guardar actual", "Editar perfil"]),
    ("it", ["Profili server", "Nome profilo", "Salva corrente", "Modifica profilo"]),
    ("ru", ["Профили сервера", "Имя профиля", "Сохранить текущий", "Редактировать профиль"]),
    ("uk", ["Профілі сервера", "Назва профілю", "Зберегти поточний", "Редагувати профіль"]),
    ("zh-cn", ["服务器配置", "配置名称", "保存当前", "编辑配置"]),
    ("zh-tw", ["伺服器設定檔", "設定檔名稱", "儲存目前設定", "編輯設定檔"]),
    ("ja", ["サーバープロファイル", "プロファイル名", "現在の設定を保存", "プロファイルを編集"]),
    ("ko", ["서버 프로필", "프로필 이름", "현재 설정 저장", "프로필 편집"]),
    ("br", ["Perfis de servidor", "Nome do perfil", "Salvar atual", "Editar perfil"]),
    ("pt", ["Perfis de servidor", "Nome do perfil", "Salvar atual", "Editar perfil"]),
    ("pt-br", ["Perfis de servidor", "Nome do perfil", "Salvar atual", "Editar perfil"]),
    ("pt-pt", ["Perfis de servidor", "Nome do perfil", "Guardar atual", "Editar perfil"]),
    ("nl", ["Serverprofielen", "Profielnaam", "Huidige opslaan", "Profiel bewerken"]),
    ("nb", ["Serverprofiler", "Profilnavn", "Lagre gjeldende", "Rediger profil"]),
    ("da", ["Serverprofiler", "Profilnavn", "Gem nuværende", "Rediger profil"]),
    ("sv", ["Serverprofiler", "Profilnamn", "Spara nuvarande", "Redigera profil"]),
    ("fi", ["Palvelinprofiilit", "Profiilin nimi", "Tallenna nykyinen", "Muokkaa profiilia"]),
    ("et", ["Serveriprofiilid", "Profiili nimi", "Salvesta praegune", "Muuda profiili"]),
    ("lt", ["Serverio profiliai", "Profilio pavadinimas", "Išsaugoti dabartinį", "Redaguoti profilį"]),
    ("lv", ["Servera profili", "Profila nosaukums", "Saglabāt pašreizējo", "Rediģēt profilu"]),
    ("pl", ["Profile serwera", "Nazwa profilu", "Zapisz bieżący", "Edytuj profil"]),
    ("cs", ["Profily serveru", "Název profilu", "Uložit aktuální", "Upravit profil"]),
    ("sk", ["Profily servera", "Názov profilu", "Uložiť aktuálny", "Upraviť profil"]),
    ("sl", ["Profili strežnika", "Ime profila", "Shrani trenutnega", "Uredi profil"]),
    ("hr", ["Profili poslužitelja", "Naziv profila", "Spremi trenutni", "Uredi profil"]),
    ("bg", ["Профили на сървъра", "Име на профила", "Запази текущия", "Редактиране на профила"]),
    ("ro", ["Profiluri de server", "Nume profil", "Salvează curent", "Editează profilul"]),
    ("el", ["Προφίλ διακομιστή", "Όνομα προφίλ", "Αποθήκευση τρέχοντος", "Επεξεργασία προφίλ"]),
    ("tr", ["Sunucu profilleri", "Profil adı", "Mevcut olanı kaydet", "Profili düzenle"]),
    ("ca", ["Perfils de servidor", "Nom del perfil", "Desa l'actual", "Edita el perfil"]),
    ("gl", ["Perfís de servidor", "Nome do perfil", "Gardar o actual", "Editar perfil"]),
    ("id", ["Profil server", "Nama profil", "Simpan saat ini", "Edit profil"]),
    ("vi", ["Hồ sơ máy chủ", "Tên hồ sơ", "Lưu hiện tại", "Chỉnh sửa hồ sơ"]),
    ("th", ["โปรไฟล์เซิร์ฟเวอร์", "ชื่อโปรไฟล์", "บันทึกปัจจุบัน", "แก้ไขโปรไฟล์"]),
    ("hi", ["सर्वर प्रोफ़ाइल", "प्रोफ़ाइल का नाम", "वर्तमान सहेजें", "प्रोफ़ाइल संपादित करें"]),
    ("ar", ["ملفات تعريف الخادم", "اسم الملف الشخصي", "حفظ الحالي", "تعديل الملف الشخصي"]),
    ("he", ["פרופילי שרת", "שם פרופיל", "שמור נוכחי", "ערוך פרופיל"]),
    ("fa", ["پروفایل‌های سرور", "نام پروفایل", "ذخیره فعلی", "ویرایش پروفایل"]),
];

pub fn get(lang: &str, name: &str) -> Option<&'static str> {
    let i = KEYS.iter().position(|k| *k == name)?;
    TABLE.iter().find(|(l, _)| *l == lang).map(|(_, t)| t[i])
}

#[cfg(test)]
mod tests {
    #[test]
    fn translates_known_languages_only() {
        assert_eq!(super::get("hu", "Server profiles"), Some("Szerverprofilok"));
        assert_eq!(super::get("xx", "Server profiles"), None);
        assert_eq!(super::get("hu", "Unrelated"), None);
    }
}
