// Translations for strings added by this fork. Kept out of src/lang/*.rs so upstream merges
// never conflict with them. Missing languages fall back to the English key.
const KEYS: [&str; 4] = [
    "Server profiles",
    "Profile name",
    "Save current",
    "Edit profile",
];

pub fn get(lang: &str, name: &str) -> Option<&'static str> {
    let i = KEYS.iter().position(|k| *k == name)?;
    let t: [&'static str; 4] = match lang {
        "hu" => [
            "Szerverprofilok",
            "Profil neve",
            "Jelenlegi mentése",
            "Profil szerkesztése",
        ],
        "de" => [
            "Serverprofile",
            "Profilname",
            "Aktuelles speichern",
            "Profil bearbeiten",
        ],
        "fr" => [
            "Profils de serveur",
            "Nom du profil",
            "Enregistrer l'actuel",
            "Modifier le profil",
        ],
        "es" => [
            "Perfiles de servidor",
            "Nombre del perfil",
            "Guardar actual",
            "Editar perfil",
        ],
        "ru" => [
            "Профили сервера",
            "Имя профиля",
            "Сохранить текущий",
            "Редактировать профиль",
        ],
        "zh-cn" => ["服务器配置", "配置名称", "保存当前", "编辑配置"],
        "br" | "pt" | "pt-br" => [
            "Perfis de servidor",
            "Nome do perfil",
            "Salvar atual",
            "Editar perfil",
        ],
        "pt-pt" => [
            "Perfis de servidor",
            "Nome do perfil",
            "Guardar atual",
            "Editar perfil",
        ],
        "ja" => [
            "サーバープロファイル",
            "プロファイル名",
            "現在の設定を保存",
            "プロファイルを編集",
        ],
        _ => return None,
    };
    Some(t[i])
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
