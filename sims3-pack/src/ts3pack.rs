use anyhow::{bail, Context, Result};
use byteorder::{LittleEndian, ReadBytesExt};
use quick_xml::events::Event;
use quick_xml::Reader;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

#[derive(Debug, Clone)]
pub struct PackagedFile {
    pub name: String,
    pub guid: String,
    pub length: u32,
    pub offset: u64,
    pub content_type: String, // lowercased
}

#[derive(Debug, Clone)]
pub struct Sims3Pack {
    pub xml: String,
    pub packaged_files: Vec<PackagedFile>,
    pub xml_length: u32,
    pub xml_offset: u64, // start of binary payloads = 17 + xml_length
}

pub fn read_sims3pack(path: impl AsRef<Path>) -> Result<Sims3Pack> {
    let path = path.as_ref();
    let mut f = File::open(path).with_context(|| format!("open {}", path.display()))?;

    let mut header = [0u8; 17];
    f.read_exact(&mut header).context("read sims3pack header")?;
    // bytes [4..11] should be "TS3Pack"
    if &header[4..11] != b"TS3Pack" {
        bail!("File is corrupted or not a Sims3Pack");
    }

    f.seek(SeekFrom::Start(13))?;
    let xml_len = f.read_u32::<LittleEndian>()?;
    let xml_offset = 17u64 + xml_len as u64;

    let mut xml_bytes = vec![0u8; xml_len as usize];
    f.read_exact(&mut xml_bytes).context("read sims3pack xml")?;
    let mut xml = String::from_utf8(xml_bytes).context("xml is not valid utf-8")?;

    let parsed = parse_packaged_files(&xml).or_else(|e| {
        // Match original behavior: if the XML parser chokes on localized blocks, strip them and retry.
        let lowered = e.to_string().to_lowercase();
        if lowered.contains("localized")
            || lowered.contains("mismatched")
            || lowered.contains("invalid")
        {
            strip_localized_blocks(&mut xml);
            parse_packaged_files(&xml)
        } else {
            Err(e)
        }
    })?;

    Ok(Sims3Pack {
        xml,
        packaged_files: parsed,
        xml_length: xml_len,
        xml_offset,
    })
}

fn strip_localized_blocks(xml: &mut String) {
    for (start_tag, end_tag) in [
        ("<LocalizedNames>", "</LocalizedNames>"),
        ("<LocalizedDescriptions>", "</LocalizedDescriptions>"),
    ] {
        if let Some(start) = xml.to_lowercase().find(&start_tag.to_lowercase()) {
            if let Some(end) = xml.to_lowercase().rfind(&end_tag.to_lowercase()) {
                let end2 = end + end_tag.len();
                if end2 > start && end2 <= xml.len() {
                    xml.replace_range(start..end2, "");
                }
            }
        }
    }
}

fn parse_packaged_files(xml: &str) -> Result<Vec<PackagedFile>> {
    let mut reader = Reader::from_str(xml);
    reader.trim_text(true);

    let mut buf = Vec::new();
    let mut in_packaged_file = false;

    let mut cur_name = None;
    let mut cur_guid = None;
    let mut cur_len = None;
    let mut cur_off = None;
    let mut cur_ct = None;

    let mut results = Vec::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(e)) => {
                let tag = String::from_utf8_lossy(e.name().as_ref()).to_ascii_lowercase();
                if tag == "packagedfile" {
                    in_packaged_file = true;
                    cur_name = None;
                    cur_guid = None;
                    cur_len = None;
                    cur_off = None;
                    cur_ct = None;
                } else if in_packaged_file {
                    // Next Text event will hold the value for this tag.
                    // We store the tag name in the reader state by peeking; simplest is to read text in the Start branch using read_text.
                    let val = reader.read_text(e.name()).unwrap_or_default();
                    match tag.as_str() {
                        "name" => cur_name = Some(val),
                        "guid" => cur_guid = Some(val),
                        "length" => cur_len = Some(val.parse::<u32>().unwrap_or(0)),
                        "offset" => cur_off = Some(val.parse::<u64>().unwrap_or(0)),
                        "contenttype" => cur_ct = Some(val.to_ascii_lowercase()),
                        _ => {}
                    }
                }
            }
            Ok(Event::End(e)) => {
                let tag = String::from_utf8_lossy(e.name().as_ref()).to_ascii_lowercase();
                if tag == "packagedfile" && in_packaged_file {
                    in_packaged_file = false;
                    let pf = PackagedFile {
                        name: cur_name.clone().unwrap_or_default().into_owned(),
                        guid: cur_guid.clone().unwrap_or_default().into_owned(),
                        length: cur_len.unwrap_or(0),
                        offset: cur_off.unwrap_or(0),
                        content_type: cur_ct.clone().unwrap_or_else(|| "unknown".to_string()),
                    };
                    results.push(pf);
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => bail!("XML parse error: {e}"),
            _ => {}
        }
        buf.clear();
    }

    Ok(results)
}

/// Reads a packaged payload by its `PackagedFile` info.
pub fn read_packaged_payload(
    path: impl AsRef<Path>,
    s3p: &Sims3Pack,
    pf: &PackagedFile,
) -> Result<Vec<u8>> {
    let path = path.as_ref();
    let mut f = File::open(path)?;
    let pos = s3p.xml_offset + pf.offset;
    f.seek(SeekFrom::Start(pos))?;
    let mut buf = vec![0u8; pf.length as usize];
    f.read_exact(&mut buf)?;
    Ok(buf)
}
