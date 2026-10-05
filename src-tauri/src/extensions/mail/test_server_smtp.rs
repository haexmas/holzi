//! The SMTP side of the test server: EHLO, AUTH PLAIN, MAIL, RCPT, DATA, QUIT.

use base64::engine::general_purpose::STANDARD;
use base64::Engine as _;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpStream;

use super::{Shared, PASSWORD, USER};

pub(super) async fn session(stream: TcpStream, state: Shared) {
    let (read, mut write) = stream.into_split();
    let mut reader = BufReader::new(read);
    let _ = write.write_all(b"220 test ESMTP\r\n").await;
    let mut recipients = Vec::new();
    loop {
        let mut line = String::new();
        if reader.read_line(&mut line).await.unwrap_or(0) == 0 {
            return;
        }
        let upper = line.trim_end().to_ascii_uppercase();
        let reply: String = if upper.starts_with("EHLO") {
            "250-test\r\n250 AUTH PLAIN\r\n".into()
        } else if let Some(token) = line.trim_end().strip_prefix("AUTH PLAIN ") {
            let decoded = STANDARD.decode(token).unwrap_or_default();
            if decoded == format!("\0{USER}\0{PASSWORD}").as_bytes() {
                "235 ok\r\n".into()
            } else {
                "535 5.7.8 authentication failed\r\n".into()
            }
        } else if upper.starts_with("MAIL FROM") {
            "250 ok\r\n".into()
        } else if upper.starts_with("RCPT TO") {
            recipients.push(
                line.trim_end()[8..]
                    .trim_matches(['<', '>', ' '])
                    .to_owned(),
            );
            "250 ok\r\n".into()
        } else if upper == "DATA" {
            let _ = write.write_all(b"354 go\r\n").await;
            let mut data = Vec::new();
            loop {
                let mut l = String::new();
                if reader.read_line(&mut l).await.unwrap_or(0) == 0 || l == ".\r\n" {
                    break;
                }
                data.extend(l.as_bytes());
            }
            state
                .lock()
                .unwrap()
                .sent
                .push((std::mem::take(&mut recipients), data));
            "250 queued\r\n".into()
        } else if upper == "QUIT" {
            let _ = write.write_all(b"221 bye\r\n").await;
            return;
        } else {
            "250 ok\r\n".into()
        };
        let _ = write.write_all(reply.as_bytes()).await;
    }
}
