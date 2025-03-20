use hbb_common::{
    compress::{compress, decompress},
    log,
    message_proto::*,
    tokio::{fs::File, io::*},
    ResultType,
};
use std::path::PathBuf;

pub struct PrinterJob {
    id: i32,
    file: File,
    path: PathBuf,
}

impl PrinterJob {
    pub async fn new_read(id: i32, path: String) -> ResultType<Self> {
        let file = File::open(&path).await?;
        Ok(Self {
            id,
            file,
            path: PathBuf::from(path),
        })
    }

    pub async fn read(
        &mut self,
        stream: &mut hbb_common::Stream,
        remove: &mut bool,
    ) -> ResultType<()> {
        const BUF_SIZE: usize = 128 * 1024;
        let mut buf: Vec<u8> = vec![0; BUF_SIZE];
        let mut offset: usize = 0;
        let mut compressed = false;
        let mut msg = Message::new();
        let mut printer = Printer::new();
        loop {
            match self.file.read(&mut buf[offset..]).await {
                Err(err) => {
                    let error = PrinterError {
                        id: self.id,
                        error: err.to_string(),
                        ..Default::default()
                    };
                    printer.set_printer_error(error);
                    msg.set_printer(printer);
                    stream.send(&msg).await?;
                    return Err(err.into());
                }
                Ok(n) => {
                    offset += n;
                    if n == 0 || offset == BUF_SIZE {
                        break;
                    }
                }
            }
        }
        unsafe { buf.set_len(offset) };
        if offset == 0 {
            let done = PrinterDone {
                id: self.id,
                ..Default::default()
            };
            printer.set_printer_done(done);
            msg.set_printer(printer);
            stream.send(&msg).await?;
        } else {
            let tmp = compress(&buf);
            if tmp.len() < buf.len() {
                buf = tmp;
                compressed = true;
            }
            let block = PrinterBlock {
                id: self.id,
                data: buf.into(),
                compressed,
                ..Default::default()
            };
            log::info!("send printer block: {:?}", block.data.len());
            printer.set_printer_block(block);
            msg.set_printer(printer);
            stream.send(&msg).await?;
            *remove = false;
        }
        Ok(())
    }

    pub async fn new_write(id: i32) -> ResultType<Self> {
        let path = std::env::temp_dir().join(format!("rustdesk_printer_input_{id}"));
        log::info!("create printer file: {:?}", path);
        let file = File::create(&path).await?;
        Ok(Self { id, file, path })
    }

    pub async fn write_block(&mut self, block: PrinterBlock) -> ResultType<()> {
        let data = if block.compressed {
            decompress(&block.data)
        } else {
            block.data.into()
        };
        self.file.write_all(&data).await?;
        Ok(())
    }

    pub fn path(&self) -> PathBuf {
        self.path.clone()
    }
}
