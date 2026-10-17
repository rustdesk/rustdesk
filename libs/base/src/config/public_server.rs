pub const API_SERVER: &str = "https://admin.rustdesk.com";
pub const OIDC_PREFIX: &str = "public-server/";

pub fn connection_token<'a>(
    server: &str,
    using_public_server: bool,
    configured_token: &'a str,
    public_token: &'a str,
) -> &'a str {
    if server != "public" {
        return "";
    }
    if using_public_server {
        configured_token
    } else {
        public_token
    }
}

#[cfg(test)]
mod tests {
    use super::connection_token;

    #[test]
    fn public_connection_uses_its_own_account_on_a_private_server() {
        assert_eq!(
            connection_token("public", false, "private-token", "public-token"),
            "public-token"
        );
        assert_eq!(connection_token("public", false, "private-token", ""), "");
        assert_eq!(
            connection_token("another.example", false, "private-token", "public-token"),
            ""
        );
    }

    #[test]
    fn public_alias_reuses_the_current_public_account() {
        assert_eq!(
            connection_token("public", true, "current-public-token", "old-public-token"),
            "current-public-token"
        );
    }
}
