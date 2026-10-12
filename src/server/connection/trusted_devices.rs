use super::{ConnAuditPrimaryAuth, ConnAuditTwoFactor, Connection};
use crate::trusted_devices::{
    authority::Unavailable,
    host::{self, LoginProof},
    DeviceInfo,
};
use base::message_proto::LoginResponse;

const MIN_CONTROLLER_VERSION: &str = "1.5.0";

impl Connection {
    pub(super) async fn begin_trusted_device(&mut self) -> bool {
        let token = std::mem::take(&mut self.lr.trusted_device_v2_token);
        if self.conn_audit_primary_auth != ConnAuditPrimaryAuth::PermanentPassword
            || !self.stream.is_secured()
            || self.require_2fa.is_none()
            || self.from_switch
            || !hbb_common::config::Config::get_bool_option(
                base::config::keys::OPTION_ENABLE_TRUSTED_DEVICES,
            )
            || hbb_common::get_version_number(&self.lr.version)
                < hbb_common::get_version_number(MIN_CONTROLLER_VERSION)
        {
            return true;
        }
        let proof = LoginProof {
            password: self.lr.password.to_vec(),
            challenge: self.hash.challenge.clone(),
            salt: self.hash.salt.clone(),
            token: token.to_vec(),
        };
        match host::begin(proof).await {
            Ok(Some((attempt, valid, totp))) => {
                self.trusted_device = attempt;
                self.require_2fa = Some(totp);
                if valid {
                    self.require_2fa = None;
                    self.set_conn_audit_two_factor(ConnAuditTwoFactor::TrustedDevice);
                }
                true
            }
            Ok(None) => true,
            Err(error) => {
                self.send_login_error(error).await;
                false
            }
        }
    }

    pub(super) async fn check_trusted_device(&mut self) -> bool {
        if let Some(attempt) = self.trusted_device.as_ref() {
            if let Err(error) = attempt.check().await {
                self.trusted_device = None;
                if self.conn_audit_two_factor == ConnAuditTwoFactor::Totp
                    && error.is::<Unavailable>()
                {
                    host::log_unavailable(&error);
                    return true;
                }
                self.send_login_error(error).await;
                self.on_close("Trusted-device authentication failed", false)
                    .await;
                return false;
            }
        }
        true
    }

    pub(super) async fn enroll_trusted_device(&mut self, response: &mut LoginResponse) -> bool {
        let Some(attempt) = self.trusted_device.take() else {
            return true;
        };
        if !response.has_peer_info()
            || self.conn_audit_primary_auth != ConnAuditPrimaryAuth::PermanentPassword
        {
            return true;
        }
        let info = DeviceInfo {
            id: self.lr.my_id.clone(),
            name: self.lr.my_name.clone(),
            platform: self.lr.my_platform.clone(),
            ..Default::default()
        };
        match attempt.finish(info).await {
            Ok(token) => {
                response.trusted_device_v2_token = token.into();
                true
            }
            Err(error) => {
                if self.conn_audit_two_factor == ConnAuditTwoFactor::Totp
                    && error.is::<Unavailable>()
                {
                    host::log_unavailable(&error);
                    return true;
                }
                self.send_login_error(error).await;
                self.on_close("Trusted-device enrollment failed", false)
                    .await;
                false
            }
        }
    }
}
