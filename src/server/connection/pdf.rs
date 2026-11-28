use super::{AuthConnType, AuthedConn, Connection, AUTHED_CONNS, CONTROL_PERMISSIONS_ARRAY};
use crate::{ipc::Data, remote_print::job};
use base::{
    config::keys,
    fs,
    message_proto::{file_transfer_send_request, Message, MessageBox},
};
use hbb_common::{
    anyhow::anyhow,
    bail,
    tokio::{sync::mpsc, time::Instant},
    ResultType,
};
use std::time::Duration;

const OFFER_LIFETIME: Duration = Duration::from_secs(60);

pub(crate) struct PdfTarget {
    id: i32,
    sender: mpsc::UnboundedSender<Data>,
}

pub(crate) fn pdf_printer_target() -> ResultType<PdfTarget> {
    let connections = AUTHED_CONNS.lock().unwrap();
    let connection = select_pdf_receiver(&connections, permitted)?;
    Ok(PdfTarget {
        id: connection.conn_id,
        sender: connection.sender.clone(),
    })
}

fn select_pdf_receiver(
    connections: &[AuthedConn],
    permitted: impl Fn(i32) -> bool,
) -> ResultType<&AuthedConn> {
    job::unique_target(connections.iter().filter(|connection| {
        connection.conn_type == AuthConnType::Remote
            && connection.pdf_printer
            && !connection.sender.is_closed()
            && !superseded(connection, connections)
            && permitted(connection.conn_id)
    }))
}

fn superseded(connection: &AuthedConn, connections: &[AuthedConn]) -> bool {
    connections.iter().any(|newer| {
        super::raii::AuthedConnID::is_newer_session_remote(
            newer,
            connection.conn_id,
            &connection.session_key,
        )
    })
}

fn permitted(id: i32) -> bool {
    let permission = CONTROL_PERMISSIONS_ARRAY
        .lock()
        .unwrap()
        .iter()
        .find(|(connection_id, _)| *connection_id == id)
        .map(|(_, permissions)| permissions.clone());
    Connection::permission(keys::OPTION_ENABLE_REMOTE_PRINTER, &permission)
}

impl PdfTarget {
    pub(crate) fn authorized(&self) -> bool {
        if self.sender.is_closed() || !permitted(self.id) {
            return false;
        }
        let connections = AUTHED_CONNS.lock().unwrap();
        connections.iter().any(|connection| {
            connection.conn_id == self.id
                && connection.pdf_printer
                && connection.sender.same_channel(&self.sender)
                && !superseded(connection, &connections)
        })
    }

    pub(crate) fn send(&self, data: Vec<u8>) -> ResultType<()> {
        if !self.authorized() {
            bail!("The bound PDF receiver disconnected, was replaced, or lost printing permission");
        }
        self.sender
            .send(Data::PdfPrinterData(data))
            .map_err(|_| anyhow!("The bound PDF receiver disconnected"))
    }

    pub(crate) fn report_error(&self, error: &str) {
        if let Err(send_error) = self.sender.send(Data::PdfPrinterError(error.to_owned())) {
            hbb_common::log::trace!(
                "PDF capture error could not reach the closed session: {send_error}"
            );
        }
    }
}

impl Connection {
    pub(super) async fn send_pdf_request(&mut self, data: Vec<u8>) {
        if !self.lr.pdf_printing_supported
            || !Self::permission(
                keys::OPTION_ENABLE_REMOTE_PRINTER,
                &self.control_permissions,
            )
        {
            self.send_remote_printing_disallowed().await;
            return;
        }
        let path = job::new_request_path();
        let mut message = fs::new_send(0, fs::JobType::Printer, path.clone(), 1, false);
        message.mut_file_action().mut_send().print_format =
            file_transfer_send_request::PrintFormat::Pdf.into();
        self.printer_data
            .retain(|(created, _, _)| created.elapsed() < OFFER_LIFETIME);
        self.printer_data.push((Instant::now(), path, data));
        self.send(message).await;
    }

    pub(super) async fn send_pdf_error(&mut self, error: String) {
        let mut message = Message::new();
        message.set_message_box(MessageBox {
            msgtype: "custom-error".to_owned(),
            title: "Remote printing".to_owned(),
            text: error,
            ..Default::default()
        });
        self.send(message).await;
    }

    pub(super) fn check_pdf_download(&self, path: &str) -> ResultType<()> {
        if !job::is_pdf_request_path(path) {
            return Ok(());
        }
        if !self.lr.pdf_printing_supported
            || !Self::permission(
                keys::OPTION_ENABLE_REMOTE_PRINTER,
                &self.control_permissions,
            )
        {
            bail!("PDF printing is no longer permitted for this session");
        }
        if !self
            .printer_data
            .iter()
            .any(|(_, pending, _)| pending == path)
        {
            bail!("The PDF print request expired or was already consumed");
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::{super::SessionKey, *};

    fn receiver(id: i32, session_id: u64) -> (AuthedConn, mpsc::UnboundedReceiver<Data>) {
        let (sender, receiver) = mpsc::unbounded_channel();
        (
            AuthedConn {
                conn_id: id,
                conn_type: AuthConnType::Remote,
                session_key: SessionKey {
                    peer_id: "peer".to_owned(),
                    name: "name".to_owned(),
                    session_id,
                },
                sender,
                printer: true,
                pdf_printer: true,
            },
            receiver,
        )
    }

    #[test]
    fn pdf_reconnect_selects_new_session_and_rejects_independent_receivers() {
        let (old, _old_rx) = receiver(1, 7);
        let (new, _new_rx) = receiver(2, 7);
        let (other, _other_rx) = receiver(3, 8);
        let mut connections = vec![old, new];
        assert_eq!(
            select_pdf_receiver(&connections, |_| true).unwrap().conn_id,
            2
        );
        connections.push(other);
        assert!(select_pdf_receiver(&connections, |_| true).is_err());
    }

    #[test]
    fn pdf_reconnect_does_not_fall_back_to_a_superseded_receiver() {
        let (old, _old_rx) = receiver(1, 7);
        let (new, _new_rx) = receiver(2, 7);
        let mut connections = vec![old, new];
        assert!(select_pdf_receiver(&connections, |id| id != 2).is_err());
        connections[1].pdf_printer = false;
        assert!(select_pdf_receiver(&connections, |_| true).is_err());
    }
}
