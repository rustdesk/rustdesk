use super::*;

#[test]
fn network_pin_verification_does_not_authorize_pin_changes() {
    let uid = 1000;
    let token = [42; 32];
    for key in [None, Some(digest(&token))] {
        let mut grants = Grants::default();
        assert!(grants.verify(uid, key, || Ok(true)).unwrap());
        assert_eq!(grants.authorized(uid, &token), key.is_some());
    }
}

#[test]
fn pin_management_requires_the_users_own_approved_token() {
    let token = [42; 32];
    let mut grants = Grants::default();
    let owner = 1000;
    assert!(!grants.authorized(owner, &token));
    assert!(!grants.admin_approve(owner, owner, digest(&token)));
    assert!(grants.admin_approve(0, owner, digest(&token)));
    assert!(!grants.authorized(owner + 1, &token));
    assert!(!grants.authorized(owner, &digest(&token)));
    assert!(grants.authorized(owner, &token));
}

#[test]
fn pin_changes_require_the_current_console_user_or_root() {
    let uid = 1000;
    let change = Request::Set(Some([42; 32]), "fixture".into());
    assert!(change.check_active_user(uid, || Some(uid)).is_ok());
    for active in [Some(uid + 1), None] {
        let error = change.check_active_user(uid, || active).unwrap_err();
        assert_eq!(error.to_string(), REQUIRED);
    }
    assert!(change.check_active_user(0, || None).is_ok());
}
