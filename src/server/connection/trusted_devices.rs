use super::{ConnAuditPrimaryAuth, ConnAuditTwoFactor, Connection};
use crate::trusted_devices::{
    authority::Unavailable,
    host::{self, LoginProof},
    DeviceInfo,
};
use base::message_proto::LoginResponse;

impl Connection {
    pub(super) async fn begin_trusted_device(&mut self) -> bool {
        if self.conn_audit_primary_auth != ConnAuditPrimaryAuth::PermanentPassword
            || !self.stream.is_secured()
            || self.require_2fa.is_none()
            || self.from_switch
            || !hbb_common::config::Config::get_bool_option(
                base::config::keys::OPTION_ENABLE_TRUSTED_DEVICES,
            )
            || (!self.lr.trusted_device_v2_supported && self.lr.hwid.is_empty())
        {
            self.lr.trusted_device_v2_token.clear();
            return true;
        }
        let result = match LoginProof::from_login(&mut self.lr, &self.hash) {
            Ok(proof) => host::begin(proof).await,
            Err(error) => Err(error),
        };
        match result {
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
        if let Some(attempt) = self.trusted_device.as_mut() {
            if let Err(error) = attempt.check().await {
                if self.conn_audit_two_factor == ConnAuditTwoFactor::Totp
                    && error.is::<Unavailable>()
                {
                    // Retain the ticket for the final authentication check.
                    attempt.enroll = false;
                    host::log_unavailable(&error);
                    return true;
                }
                self.trusted_device = None;
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
                response.trusted_device_v2_supported = self.lr.trusted_device_v2_supported;
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
