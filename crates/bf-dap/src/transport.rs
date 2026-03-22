use std::io::{BufRead, Write};

pub struct DapTransport {
    reader: Box<dyn BufRead>,
    writer: Box<dyn Write>,
}

impl DapTransport {
    pub fn stdio() -> Self {
        DapTransport {
            reader: Box::new(std::io::BufReader::new(std::io::stdin())),
            writer: Box::new(std::io::BufWriter::new(std::io::stdout())),
        }
    }

    pub fn from_reader_writer(
        reader: impl BufRead + 'static,
        writer: impl Write + 'static,
    ) -> Self {
        DapTransport {
            reader: Box::new(reader),
            writer: Box::new(writer),
        }
    }

    pub fn read_message(&mut self) -> Option<serde_json::Value> {
        let mut content_length: Option<usize> = None;
        loop {
            let mut line = String::new();
            match self.reader.read_line(&mut line) {
                Ok(0) => return None,
                Ok(_) => {}
                Err(_) => return None,
            }
            let trimmed = line.trim_end_matches(|c| c == '\r' || c == '\n');
            if trimmed.is_empty() {
                break;
            }
            if let Some(rest) = trimmed.strip_prefix("Content-Length: ") {
                content_length = rest.trim().parse::<usize>().ok();
            }
        }

        let length = content_length?;
        let mut body = vec![0u8; length];
        self.reader.read_exact(&mut body).ok()?;
        serde_json::from_slice(&body).ok()
    }

    pub fn send_message(&mut self, msg: &serde_json::Value) {
        let body = msg.to_string();
        let header = format!("Content-Length: {}\r\n\r\n", body.len());
        let _ = self.writer.write_all(header.as_bytes());
        let _ = self.writer.write_all(body.as_bytes());
        let _ = self.writer.flush();
    }
}
