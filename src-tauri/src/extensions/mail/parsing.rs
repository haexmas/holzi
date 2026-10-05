//! A fetched RFC 822 message as the SDK's `MailMessage`, through `mail-parser`.

use async_imap::types::{Fetch, Flag};
use mail_parser::{Address as ParsedAddress, MessageParser, MimeHeaders};

use super::{Address, Attachment, MailError, Message, MessageEnvelope};

fn parsed(rfc822: &[u8]) -> Result<mail_parser::Message<'_>, MailError> {
    MessageParser::default()
        .parse(rfc822)
        .ok_or_else(|| MailError::Parse("not a message".into()))
}

fn addresses(header: Option<&ParsedAddress<'_>>) -> Vec<Address> {
    header
        .map(|h| {
            h.iter()
                .filter_map(|a| {
                    let email = a.address()?.to_owned();
                    (!email.is_empty()).then(|| Address {
                        name: a.name().map(str::to_owned),
                        email,
                    })
                })
                .collect()
        })
        .unwrap_or_default()
}

/// The whole message; `fetch` brings what IMAP knows beside the bytes (UID, flags, dates).
pub fn parse_message(rfc822: &[u8], fetch: &Fetch) -> Result<Message, MailError> {
    let message = parsed(rfc822)?;
    let attachments: Vec<Attachment> = message
        .attachments()
        .enumerate()
        .map(|(index, part)| Attachment {
            part_index: index as u32,
            filename: part.attachment_name().map(str::to_owned),
            content_type: part
                .content_type()
                .map(|ct| match ct.subtype() {
                    Some(sub) => format!("{}/{sub}", ct.ctype()),
                    None => ct.ctype().to_owned(),
                })
                .unwrap_or_else(|| "application/octet-stream".to_owned()),
            size: part.contents().len() as u64,
            content_id: part.content_id().map(str::to_owned),
            is_inline: part.content_disposition().is_some_and(|d| d.is_inline()),
        })
        .collect();
    Ok(Message {
        envelope: MessageEnvelope {
            uid: fetch
                .uid
                .ok_or_else(|| MailError::Parse("no UID in the answer".into()))?,
            flags: fetch.flags().map(|f| flag_name(&f)).collect(),
            internal_date: fetch.internal_date().map(|d| d.timestamp()),
            subject: message.subject().map(str::to_owned),
            from: addresses(message.from()),
            to: addresses(message.to()),
            cc: addresses(message.cc()),
            message_id: message.message_id().map(str::to_owned),
            in_reply_to: message.in_reply_to().as_text().map(str::to_owned),
            references: message
                .references()
                .as_text_list()
                .map(|l| l.iter().map(|s| s.to_string()).collect())
                .unwrap_or_default(),
            size: fetch.size,
            has_attachments: !attachments.is_empty(),
        },
        body_text: message.body_text(0).map(|c| c.into_owned()),
        body_html: message.body_html(0).map(|c| c.into_owned()),
        attachments,
    })
}

/// The bytes of attachment `part_index`, counted as [`parse_message`] counts them.
pub fn extract_attachment(rfc822: &[u8], part_index: u32) -> Result<Vec<u8>, MailError> {
    parsed(rfc822)?
        .attachments()
        .nth(part_index as usize)
        .map(|part| part.contents().to_vec())
        .ok_or(MailError::NotFound)
}

/// The `References` header of the extra header section of an envelope fetch, unfolded.
pub fn references_header(fetch: &Fetch) -> Vec<String> {
    let Some(text) = fetch.header().and_then(|h| std::str::from_utf8(h).ok()) else {
        return Vec::new();
    };
    let mut lines: Vec<String> = Vec::new();
    for line in text.lines() {
        if line.starts_with([' ', '\t']) {
            if let Some(last) = lines.last_mut() {
                last.push(' ');
                last.push_str(line.trim_start());
                continue;
            }
        }
        lines.push(line.to_owned());
    }
    lines
        .iter()
        .find_map(|line| {
            let (name, value) = line.split_once(':')?;
            name.eq_ignore_ascii_case("references").then(|| {
                value
                    .split_whitespace()
                    .map(|t| t.trim_matches(['<', '>']).to_owned())
                    .filter(|t| !t.is_empty())
                    .collect()
            })
        })
        .unwrap_or_default()
}

/// A flag as IMAP writes it (`\Seen`, a keyword as it is).
pub fn flag_name(flag: &Flag<'_>) -> String {
    match flag {
        Flag::Seen => "\\Seen".into(),
        Flag::Answered => "\\Answered".into(),
        Flag::Flagged => "\\Flagged".into(),
        Flag::Deleted => "\\Deleted".into(),
        Flag::Draft => "\\Draft".into(),
        Flag::Recent => "\\Recent".into(),
        Flag::MayCreate => "\\*".into(),
        Flag::Custom(keyword) => keyword.to_string(),
    }
}
