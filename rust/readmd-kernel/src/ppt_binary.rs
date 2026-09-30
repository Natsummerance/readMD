//! Native PowerPoint 97-2003 (`.ppt`) text reader.
//!
//! Walks the `PowerPoint Document` stream as a record tree.  Slide text lives
//! in `SlideListWithText` containers (one per list instance: slides, masters,
//! notes), where a `SlidePersistAtom` starts each slide and `TextHeaderAtom`
//! tells titles from body text; `TextCharsAtom` is UTF-16LE and
//! `TextBytesAtom` is 8-bit (Latin-1 per the spec).  Text stored in drawing
//! shapes of the slide records themselves (PowerPoint 2003+ often puts it
//! there instead) is collected per `Slide` container as a fallback, and notes
//! pages become a `### Notes` section — matching the `.pptx` output shape.

use crate::convert::CfbReader;

const MAX_OUT: usize = 64 << 20;

const RT_DOCUMENT: u16 = 0x03E8;
const RT_SLIDE: u16 = 0x03EE;
const RT_NOTES: u16 = 0x03F0;
const RT_SLIDE_LIST_WITH_TEXT: u16 = 0x0FF0;
const RT_SLIDE_PERSIST_ATOM: u16 = 0x03F3;
const RT_TEXT_HEADER_ATOM: u16 = 0x0F9F;
const RT_TEXT_CHARS_ATOM: u16 = 0x0FA0;
const RT_TEXT_BYTES_ATOM: u16 = 0x0FA8;
const RT_CSTRING: u16 = 0x0FBA;

#[derive(Clone, Copy)]
struct Hdr {
    ver_inst: u16,
    typ: u16,
    len: usize,
}

fn hdr(d: &[u8], o: usize) -> Option<Hdr> {
    let b = d.get(o..o + 8)?;
    Some(Hdr {
        ver_inst: u16::from_le_bytes([b[0], b[1]]),
        typ: u16::from_le_bytes([b[2], b[3]]),
        len: u32::from_le_bytes([b[4], b[5], b[6], b[7]]) as usize,
    })
}

impl Hdr {
    fn is_container(&self) -> bool {
        self.ver_inst & 0x000F == 0x000F
    }
    fn instance(&self) -> u16 {
        self.ver_inst >> 4
    }
}

/// Visit child records of `d[start..end]`, bounded and non-recursive in
/// memory use (the callback decides whether to descend).
fn children(d: &[u8], start: usize, end: usize) -> Vec<(Hdr, usize, usize)> {
    let mut out = Vec::new();
    let end = end.min(d.len());
    let mut p = start;
    while p + 8 <= end && out.len() < 1_000_000 {
        let Some(h) = hdr(d, p) else { break };
        let body = p + 8;
        let body_end = body.saturating_add(h.len).min(end);
        out.push((h, body, body_end));
        if body_end <= p {
            break;
        }
        p = body_end;
    }
    out
}

fn text_of(h: &Hdr, d: &[u8]) -> Option<String> {
    let s = match h.typ {
        RT_TEXT_CHARS_ATOM => {
            let units: Vec<u16> = d.chunks_exact(2).map(|c| u16::from_le_bytes([c[0], c[1]])).collect();
            String::from_utf16_lossy(&units)
        }
        RT_TEXT_BYTES_ATOM => d.iter().map(|&b| b as char).collect(),
        _ => return None,
    };
    // Vertical tab = soft line break; CR = paragraph; strip other controls.
    let s: String = s
        .chars()
        .map(|c| match c {
            '\r' | '\u{b}' => '\n',
            c if (c as u32) < 0x20 && c != '\n' && c != '\t' => ' ',
            c => c,
        })
        .collect();
    Some(s)
}

#[derive(Default)]
struct SlideText {
    title: Vec<String>,
    body: Vec<String>,
}

impl SlideText {
    fn push(&mut self, text_type: Option<u32>, s: String) {
        let s = s.trim().to_string();
        if s.is_empty() || s == "*" {
            return;
        }
        match text_type {
            Some(0) | Some(6) => self.title.push(s),
            _ => self.body.push(s),
        }
    }
    fn is_empty(&self) -> bool {
        self.title.is_empty() && self.body.is_empty()
    }
}

/// Collect text atoms inside a container (slide / notes drawing).
fn collect_atoms(d: &[u8], start: usize, end: usize, into: &mut SlideText, depth: u32) {
    if depth > 32 {
        return;
    }
    let mut tt: Option<u32> = None;
    for (h, b, e) in children(d, start, end) {
        if h.is_container() {
            collect_atoms(d, b, e, into, depth + 1);
        } else if h.typ == RT_TEXT_HEADER_ATOM {
            tt = d.get(b..b + 4).map(|x| u32::from_le_bytes([x[0], x[1], x[2], x[3]]));
        } else if let Some(s) = text_of(&h, &d[b..e]) {
            into.push(tt.take(), s);
        }
    }
}

fn render_slide(n: usize, s: &SlideText, notes: Option<&SlideText>, out: &mut Vec<String>) {
    match s.title.first() {
        Some(t) => out.push(format!("## {}", t.replace('\n', " "))),
        None => out.push(format!("## Slide {n}")),
    }
    out.push(String::new());
    for extra in s.title.iter().skip(1).chain(s.body.iter()) {
        for para in extra.split('\n').map(str::trim).filter(|p| !p.is_empty()) {
            out.push(para.to_string());
            out.push(String::new());
        }
    }
    if let Some(nt) = notes.filter(|x| !x.is_empty()) {
        out.push("### Notes".into());
        for p in nt.title.iter().chain(nt.body.iter()) {
            out.push(p.trim().to_string());
        }
        out.push(String::new());
    }
}

pub fn ppt_to_md(data: &[u8]) -> Result<String, String> {
    let cfb = CfbReader::parse(data).ok_or("不是有效的 OLE2 复合文档")?;
    let doc = cfb.get_stream("PowerPoint Document").ok_or("缺少 PowerPoint Document 数据流")?;
    if cfb.get_stream("EncryptedSummary").is_some() {
        return Err("演示文稿已加密，无法读取".into());
    }
    stream_to_md(&doc)
}

fn stream_to_md(d: &[u8]) -> Result<String, String> {
    let mut slides_list: Vec<SlideText> = Vec::new();
    let mut notes_list: Vec<SlideText> = Vec::new();
    let mut slide_records: Vec<SlideText> = Vec::new();
    let mut notes_records: Vec<SlideText> = Vec::new();

    let mut walk = vec![(0usize, d.len(), 0u32)];
    while let Some((s, e, depth)) = walk.pop() {
        if depth > 8 {
            continue;
        }
        for (h, b, be) in children(d, s, e) {
            match h.typ {
                RT_DOCUMENT => walk.push((b, be, depth + 1)),
                RT_SLIDE_LIST_WITH_TEXT => {
                    // instance 0 = slides, 1 = masters, 2 = notes.
                    let target = match h.instance() {
                        0 => Some(&mut slides_list),
                        2 => Some(&mut notes_list),
                        _ => None,
                    };
                    let Some(list) = target else { continue };
                    let mut tt: Option<u32> = None;
                    for (ch, cb, ce) in children(d, b, be) {
                        match ch.typ {
                            RT_SLIDE_PERSIST_ATOM => list.push(SlideText::default()),
                            RT_TEXT_HEADER_ATOM => {
                                tt = d.get(cb..cb + 4).map(|x| u32::from_le_bytes([x[0], x[1], x[2], x[3]]))
                            }
                            _ => {
                                if let Some(t) = text_of(&ch, &d[cb..ce]) {
                                    if list.is_empty() {
                                        list.push(SlideText::default());
                                    }
                                    list.last_mut().unwrap().push(tt.take(), t);
                                }
                            }
                        }
                    }
                }
                RT_SLIDE => {
                    let mut st = SlideText::default();
                    collect_atoms(d, b, be, &mut st, 0);
                    slide_records.push(st);
                }
                RT_NOTES => {
                    let mut st = SlideText::default();
                    collect_atoms(d, b, be, &mut st, 0);
                    notes_records.push(st);
                }
                RT_CSTRING => {}
                _ if h.is_container() && depth < 2 => walk.push((b, be, depth + 1)),
                _ => {}
            }
        }
    }

    // Prefer the SlideListWithText view; fall back to text found in the
    // slide drawings when the list is empty (common in newer writers).
    let list_has_text = slides_list.iter().any(|s| !s.is_empty());
    let slides: Vec<SlideText> = if list_has_text {
        // Merge drawing-only text for slides whose list entry is empty.
        slides_list
            .into_iter()
            .enumerate()
            .map(|(k, s)| if s.is_empty() { slide_records.get_mut(k).map(std::mem::take).unwrap_or(s) } else { s })
            .collect()
    } else {
        slide_records
    };
    let notes = if notes_list.iter().any(|s| !s.is_empty()) { notes_list } else { notes_records };

    let mut out: Vec<String> = Vec::new();
    for (k, s) in slides.iter().enumerate() {
        if s.is_empty() && notes.get(k).map(|n| n.is_empty()).unwrap_or(true) {
            continue;
        }
        render_slide(k + 1, s, notes.get(k), &mut out);
        if out.iter().map(|l| l.len() + 1).sum::<usize>() > MAX_OUT {
            return Err("演示文稿文本超过 64 MiB 上限".into());
        }
    }
    if out.is_empty() {
        return Err("演示文稿中没有可提取的文字".into());
    }
    Ok(out.join("\n").trim().to_string() + "\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn atom(ver_inst: u16, typ: u16, body: &[u8]) -> Vec<u8> {
        let mut v = ver_inst.to_le_bytes().to_vec();
        v.extend(typ.to_le_bytes());
        v.extend((body.len() as u32).to_le_bytes());
        v.extend_from_slice(body);
        v
    }

    fn chars(s: &str) -> Vec<u8> {
        s.encode_utf16().flat_map(|u| u.to_le_bytes()).collect()
    }

    fn fixture() -> Vec<u8> {
        let mut list = Vec::new();
        list.extend(atom(0, RT_SLIDE_PERSIST_ATOM, &[0u8; 20]));
        list.extend(atom(0, RT_TEXT_HEADER_ATOM, &0u32.to_le_bytes()));
        list.extend(atom(0, RT_TEXT_CHARS_ATOM, &chars("季度汇报")));
        list.extend(atom(0, RT_TEXT_HEADER_ATOM, &1u32.to_le_bytes()));
        list.extend(atom(0, RT_TEXT_BYTES_ATOM, b"Revenue up\rCosts down"));
        list.extend(atom(0, RT_SLIDE_PERSIST_ATOM, &[0u8; 20]));
        list.extend(atom(0, RT_TEXT_HEADER_ATOM, &1u32.to_le_bytes()));
        list.extend(atom(0, RT_TEXT_BYTES_ATOM, b"Only body"));
        let mut notes = Vec::new();
        notes.extend(atom(0, RT_SLIDE_PERSIST_ATOM, &[0u8; 20]));
        notes.extend(atom(0, RT_TEXT_HEADER_ATOM, &2u32.to_le_bytes()));
        notes.extend(atom(0, RT_TEXT_CHARS_ATOM, &chars("讲者备注")));
        let mut doc_body = atom(0x000F, RT_SLIDE_LIST_WITH_TEXT, &list);
        doc_body.extend(atom(0x002F, RT_SLIDE_LIST_WITH_TEXT, &notes));
        atom(0x000F, RT_DOCUMENT, &doc_body)
    }

    #[test]
    fn slide_list_text_becomes_markdown() {
        let md = stream_to_md(&fixture()).unwrap();
        assert_eq!(
            md,
            "## 季度汇报\n\nRevenue up\n\nCosts down\n\n### Notes\n讲者备注\n\n## Slide 2\n\nOnly body\n"
        );
    }

    #[test]
    fn empty_stream_is_an_error() {
        assert!(stream_to_md(&[]).is_err());
        assert!(ppt_to_md(b"not ole").is_err());
    }

    #[test]
    fn garbage_never_panics() {
        let good = fixture();
        let mut seed = 0x2545_F491_4F6C_DD1Du64;
        for n in 0..500 {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            let mut v = good.clone();
            v.truncate((seed as usize) % (good.len() + 1));
            if n % 2 == 1 && !v.is_empty() {
                let at = (seed >> 24) as usize % v.len();
                v[at] = (seed >> 3) as u8;
            }
            let _ = stream_to_md(&v);
        }
    }
}
