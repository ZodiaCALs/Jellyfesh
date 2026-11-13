//! Gapless information in MP4 (ADR-0008, spike S1).
//!
//! Symphonia's MP4 demuxer parses the edit list (`elst`) without applying it and never reads
//! iTunes' `iTunSMPB` tag, so AAC in MP4 keeps its encoder priming (typically 1024 or 2112
//! frames) and its padding. This module reads both from the file itself; the decoder then drops
//! the priming and the padding.
//!
//! * `iTunSMPB` (a freeform `----` tag in `moov/udta/meta/ilst`, written by Apple's encoders):
//!   hexadecimal fields `reserved delay padding length …`.
//! * The edit list of the audio track: the first edit's media time is the delay (in media
//!   timescale units), its duration the playable length (in movie timescale units, which is
//!   often milliseconds). When the sample table (`stts`) totals the delay plus a length within
//!   that rounding, as ffmpeg writes it, the sample table's exact length is used.
//!
//! `iTunSMPB` wins when both are present and consistent with the sample table.

use std::io::{self, Read, Seek, SeekFrom};

/// Largest `moov` box read into memory.
const MAX_MOOV_BYTES: u64 = 64 << 20;
/// Top-level boxes scanned before giving up.
const MAX_TOP_LEVEL_BOXES: usize = 1024;

/// Where a trim came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TrimSource {
    ITunSmpb,
    EditList,
}

/// Encoder delay and playable length of an MP4 audio track, in frames at its sample rate.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Mp4Trim {
    /// Frames to drop at the start (encoder priming).
    pub delay: u64,
    /// Playable frames after the delay; what follows is padding.
    pub valid: u64,
    pub source: TrimSource,
}

/// What the engine needs from an MP4 file besides what Symphonia reports.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct Mp4Info {
    pub trim: Option<Mp4Trim>,
    /// Size of the media data (`mdat` payloads), for the average bit rate.
    pub mdat_bytes: Option<u64>,
}

/// Reads the gapless information of the first audio track. `sample_rate` is the track's rate as
/// Symphonia reports it. Malformed or missing boxes give `trim: None`, never an error; I/O
/// errors are returned.
pub(crate) fn read_info<R: Read + Seek>(reader: &mut R, sample_rate: u32) -> io::Result<Mp4Info> {
    let end = reader.seek(SeekFrom::End(0)).ok();
    let mut pos = 0u64;
    let mut moov = None;
    let mut mdat_bytes: Option<u64> = None;
    for _ in 0..MAX_TOP_LEVEL_BOXES {
        if end.is_some_and(|end| pos + 8 > end) {
            break;
        }
        reader.seek(SeekFrom::Start(pos))?;
        let mut header = [0u8; 16];
        if !read_full(reader, &mut header[..8])? {
            break;
        }
        let kind = [header[4], header[5], header[6], header[7]];
        let (size, header_len) = match u32::from_be_bytes([header[0], header[1], header[2], header[3]]) {
            1 => {
                if !read_full(reader, &mut header[8..16])? {
                    break;
                }
                (u64::from_be_bytes(header[8..16].try_into().expect("8 bytes")), 16)
            }
            0 => match end {
                Some(end) => (end - pos, 8),
                None => break,
            },
            size => (u64::from(size), 8),
        };
        if size < header_len {
            break;
        }
        let payload = size - header_len;
        match &kind {
            b"moov" if payload <= MAX_MOOV_BYTES => {
                let mut data = vec![0u8; payload as usize];
                reader.read_exact(&mut data)?;
                moov = Some(data);
            }
            b"mdat" => mdat_bytes = Some(mdat_bytes.unwrap_or(0) + payload),
            _ => {}
        }
        if moov.is_some() && mdat_bytes.is_some() {
            break;
        }
        pos = match pos.checked_add(size) {
            Some(next) => next,
            None => break,
        };
    }
    let trim = moov.as_deref().and_then(|moov| parse_moov(moov, sample_rate));
    Ok(Mp4Info { trim, mdat_bytes })
}

/// Fills `buf` completely; `false` at a clean end of stream before the first byte.
fn read_full<R: Read>(reader: &mut R, buf: &mut [u8]) -> io::Result<bool> {
    let mut filled = 0;
    while filled < buf.len() {
        match reader.read(&mut buf[filled..]) {
            Ok(0) if filled == 0 => return Ok(false),
            Ok(0) => return Err(io::ErrorKind::UnexpectedEof.into()),
            Ok(n) => filled += n,
            Err(err) if err.kind() == io::ErrorKind::Interrupted => {}
            Err(err) => return Err(err),
        }
    }
    Ok(true)
}

/// The boxes directly inside `data`: (type, payload).
fn boxes(data: &[u8]) -> impl Iterator<Item = ([u8; 4], &[u8])> {
    let mut rest = data;
    std::iter::from_fn(move || {
        if rest.len() < 8 {
            return None;
        }
        let size = u32::from_be_bytes(rest[..4].try_into().ok()?);
        let kind: [u8; 4] = rest[4..8].try_into().ok()?;
        let (size, header) = match size {
            0 => (rest.len(), 8),
            1 => {
                let large = u64::from_be_bytes(rest.get(8..16)?.try_into().ok()?);
                (usize::try_from(large).ok()?, 16)
            }
            size => (size as usize, 8),
        };
        if size < header || size > rest.len() {
            return None;
        }
        let payload = &rest[header..size];
        rest = &rest[size..];
        Some((kind, payload))
    })
}

fn child<'a>(data: &'a [u8], kind: &[u8; 4]) -> Option<&'a [u8]> {
    boxes(data).find(|(k, _)| k == kind).map(|(_, payload)| payload)
}

fn be_u32(data: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_be_bytes(data.get(at..at + 4)?.try_into().ok()?))
}

fn be_u64(data: &[u8], at: usize) -> Option<u64> {
    Some(u64::from_be_bytes(data.get(at..at + 8)?.try_into().ok()?))
}

/// The audio track's facts, in its own units.
#[derive(Debug, Default)]
struct AudioTrack {
    media_timescale: u32,
    /// Sum of the sample table's durations, in media timescale units.
    stts_total: Option<u64>,
    /// First non-empty edit: (segment duration in movie units, media time in media units).
    edit: Option<(u64, i64)>,
}

fn parse_moov(moov: &[u8], sample_rate: u32) -> Option<Mp4Trim> {
    if sample_rate == 0 {
        return None;
    }
    let movie_timescale = child(moov, b"mvhd").and_then(|mvhd| {
        // version 0: flags, creation (4), modification (4), timescale; version 1: 8-byte times.
        match mvhd.first()? {
            0 => be_u32(mvhd, 12),
            1 => be_u32(mvhd, 20),
            _ => None,
        }
    });
    let track = boxes(moov)
        .filter(|(kind, _)| kind == b"trak")
        .find_map(|(_, trak)| audio_track(trak))?;
    let rate = u64::from(sample_rate);
    let to_frames = |units: u64, timescale: u32| -> Option<u64> {
        let timescale = u64::from(timescale);
        (timescale > 0).then(|| {
            ((u128::from(units) * u128::from(rate) + u128::from(timescale) / 2) / u128::from(timescale))
                as u64
        })
    };
    let total = track
        .stts_total
        .and_then(|units| to_frames(units, track.media_timescale));

    if let Some(smpb) = itunsmpb(moov)
        && smpb.valid > 0
        && total.is_none_or(|total| smpb.delay + smpb.valid <= total)
    {
        return Some(smpb);
    }

    let (segment, media_time) = track.edit?;
    let delay = to_frames(u64::try_from(media_time).ok()?, track.media_timescale)?;
    let from_segment = movie_timescale.and_then(|ts| to_frames(segment, ts));
    let from_table = total.and_then(|total| total.checked_sub(delay));
    let valid = match (from_segment, from_table) {
        // The edit's duration is rounded to the movie timescale; the sample table is exact when
        // its last sample's duration was trimmed (ffmpeg does that).
        (Some(seg), Some(table)) => {
            let rounding = movie_timescale.map_or(0, |ts| rate.div_ceil(u64::from(ts.max(1))));
            if seg == 0 || seg.abs_diff(table) <= rounding {
                table
            } else {
                seg.min(table)
            }
        }
        (Some(seg), None) if seg > 0 => seg,
        (_, Some(table)) => table,
        _ => return None,
    };
    (valid > 0).then_some(Mp4Trim {
        delay,
        valid,
        source: TrimSource::EditList,
    })
}

fn audio_track(trak: &[u8]) -> Option<AudioTrack> {
    let mdia = child(trak, b"mdia")?;
    let hdlr = child(mdia, b"hdlr")?;
    if hdlr.get(8..12)? != b"soun" {
        return None;
    }
    let mdhd = child(mdia, b"mdhd")?;
    let media_timescale = match mdhd.first()? {
        0 => be_u32(mdhd, 12)?,
        1 => be_u32(mdhd, 20)?,
        _ => return None,
    };
    let stts_total = child(mdia, b"minf")
        .and_then(|minf| child(minf, b"stbl"))
        .and_then(|stbl| child(stbl, b"stts"))
        .and_then(|stts| {
            let count = be_u32(stts, 4)? as usize;
            let mut total = 0u64;
            for i in 0..count {
                let at = 8 + i * 8;
                let samples = u64::from(be_u32(stts, at)?);
                let delta = u64::from(be_u32(stts, at + 4)?);
                total = total.checked_add(samples.checked_mul(delta)?)?;
            }
            Some(total)
        });
    let edit = child(trak, b"edts")
        .and_then(|edts| child(edts, b"elst"))
        .and_then(|elst| {
            let version = *elst.first()?;
            let count = be_u32(elst, 4)? as usize;
            let entry_len = if version == 1 { 20 } else { 12 };
            (0..count).find_map(|i| {
                let at = 8 + i * entry_len;
                let (segment, media_time) = if version == 1 {
                    (be_u64(elst, at)?, be_u64(elst, at + 8)? as i64)
                } else {
                    (
                        u64::from(be_u32(elst, at)?),
                        i64::from(be_u32(elst, at + 4)? as i32),
                    )
                };
                // A media time of -1 is an empty edit (presentation delay), not audio.
                (media_time >= 0).then_some((segment, media_time))
            })
        });
    Some(AudioTrack {
        media_timescale,
        stts_total,
        edit,
    })
}

/// `iTunSMPB` from `moov/udta/meta/ilst`.
fn itunsmpb(moov: &[u8]) -> Option<Mp4Trim> {
    let meta = child(child(moov, b"udta")?, b"meta")?;
    // ISO `meta` is a full box (version and flags first); QuickTime's has its children directly.
    let meta = if meta.get(4..8) == Some(b"hdlr") {
        meta
    } else {
        meta.get(4..)?
    };
    let ilst = child(meta, b"ilst")?;
    boxes(ilst)
        .filter(|(kind, _)| kind == b"----")
        .find_map(|(_, tag)| {
            let mut name = None;
            let mut value = None;
            for (kind, payload) in boxes(tag) {
                match &kind {
                    // `name`: version and flags, then the text.
                    b"name" => name = payload.get(4..),
                    // `data`: type indicator (4) and locale (4), then the text.
                    b"data" => value = payload.get(8..),
                    _ => {}
                }
            }
            if !name?.eq_ignore_ascii_case(b"iTunSMPB") {
                return None;
            }
            parse_itunsmpb(std::str::from_utf8(value?).ok()?)
        })
}

/// ` 00000000 00000840 000001CA 00000000000AC44F …`: reserved, delay, padding, length (hex).
fn parse_itunsmpb(text: &str) -> Option<Mp4Trim> {
    let mut fields = text.split_whitespace().map(|f| u64::from_str_radix(f, 16));
    let _reserved = fields.next()?.ok()?;
    let delay = fields.next()?.ok()?;
    let _padding = fields.next()?.ok()?;
    let valid = fields.next()?.ok()?;
    Some(Mp4Trim {
        delay,
        valid,
        source: TrimSource::ITunSmpb,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    fn mp4_box(kind: &[u8; 4], payload: &[u8]) -> Vec<u8> {
        let mut out = ((payload.len() + 8) as u32).to_be_bytes().to_vec();
        out.extend_from_slice(kind);
        out.extend_from_slice(payload);
        out
    }

    fn full_box(kind: &[u8; 4], version: u8, body: &[u8]) -> Vec<u8> {
        let mut payload = vec![version, 0, 0, 0];
        payload.extend_from_slice(body);
        mp4_box(kind, &payload)
    }

    fn timescale_box(kind: &[u8; 4], timescale: u32, duration: u32) -> Vec<u8> {
        let mut body = vec![0u8; 8];
        body.extend_from_slice(&timescale.to_be_bytes());
        body.extend_from_slice(&duration.to_be_bytes());
        full_box(kind, 0, &body)
    }

    /// A minimal file: `mdat`, then `moov` with one audio track.
    fn file(edit: Option<(u32, i32)>, stts: &[(u32, u32)], smpb: Option<&str>) -> Vec<u8> {
        let mut hdlr = vec![0u8; 4];
        hdlr.extend_from_slice(b"soun");
        hdlr.extend_from_slice(&[0; 12]);
        let mut stts_body = (stts.len() as u32).to_be_bytes().to_vec();
        for (count, delta) in stts {
            stts_body.extend_from_slice(&count.to_be_bytes());
            stts_body.extend_from_slice(&delta.to_be_bytes());
        }
        let stbl = mp4_box(b"stbl", &full_box(b"stts", 0, &stts_body));
        let minf = mp4_box(b"minf", &stbl);
        let mut mdia = timescale_box(b"mdhd", 44_100, 0);
        mdia.extend(full_box(b"hdlr", 0, &hdlr));
        mdia.extend(minf);
        let mut trak = Vec::new();
        if let Some((segment, media_time)) = edit {
            let mut elst = 1u32.to_be_bytes().to_vec();
            elst.extend_from_slice(&segment.to_be_bytes());
            elst.extend_from_slice(&media_time.to_be_bytes());
            elst.extend_from_slice(&[0, 1, 0, 0]);
            trak.extend(mp4_box(b"edts", &full_box(b"elst", 0, &elst)));
        }
        trak.extend(mp4_box(b"mdia", &mdia));
        let mut moov = timescale_box(b"mvhd", 1_000, 0);
        moov.extend(mp4_box(b"trak", &trak));
        if let Some(text) = smpb {
            let mut tag = full_box(b"mean", 0, b"com.apple.iTunes");
            tag.extend(full_box(b"name", 0, b"iTunSMPB"));
            let mut data = vec![0, 0, 0, 1, 0, 0, 0, 0];
            data.extend_from_slice(text.as_bytes());
            tag.extend(mp4_box(b"data", &data));
            let ilst = mp4_box(b"ilst", &mp4_box(b"----", &tag));
            let meta = full_box(b"meta", 0, &ilst);
            moov.extend(mp4_box(b"udta", &meta));
        }
        let mut out = mp4_box(b"ftyp", b"M4A \0\0\0\0");
        out.extend(mp4_box(b"mdat", &[0u8; 100]));
        out.extend(mp4_box(b"moov", &moov));
        out
    }

    fn info(bytes: Vec<u8>) -> Mp4Info {
        read_info(&mut Cursor::new(bytes), 44_100).unwrap()
    }

    #[test]
    fn edit_list_with_an_exact_sample_table() {
        // ffmpeg: 1024 frames of priming, 66 150 playable, last sample trimmed to 614.
        let info = info(file(Some((1_500, 1_024)), &[(65, 1_024), (1, 614)], None));
        assert_eq!(
            info.trim,
            Some(Mp4Trim {
                delay: 1_024,
                valid: 66_150,
                source: TrimSource::EditList
            })
        );
        assert_eq!(info.mdat_bytes, Some(100));
    }

    #[test]
    fn edit_list_rounded_to_milliseconds_is_refined_by_the_sample_table() {
        // 26 461 frames are 600.02 ms: the edit says 600, the sample table is exact.
        let total = 1_024 + 26_461;
        let info = info(file(
            Some((600, 1_024)),
            &[(26, 1_024), (1, (total - 26 * 1_024) as u32)],
            None,
        ));
        assert_eq!(info.trim.map(|t| (t.delay, t.valid)), Some((1_024, 26_461)));
    }

    #[test]
    fn edit_list_without_a_trimmed_sample_table_uses_the_edit() {
        // Every sample is 1024 long: the table includes the padding, the edit does not.
        let info = info(file(Some((500, 2_112)), &[(25, 1_024)], None));
        assert_eq!(info.trim.map(|t| (t.delay, t.valid)), Some((2_112, 22_050)));
    }

    #[test]
    fn itunsmpb_wins_over_the_edit_list() {
        let smpb = " 00000000 00000840 000001C0 0000000000005622 00000000 00000000";
        let info = info(file(Some((500, 1_024)), &[(25, 1_024)], Some(smpb)));
        assert_eq!(
            info.trim,
            Some(Mp4Trim {
                delay: 0x840,
                valid: 0x5622,
                source: TrimSource::ITunSmpb
            })
        );
    }

    #[test]
    fn an_impossible_itunsmpb_falls_back_to_the_edit_list() {
        let smpb = " 00000000 00000840 000001C0 0000000000FFFFFF";
        let info = info(file(Some((500, 1_024)), &[(25, 1_024)], Some(smpb)));
        assert_eq!(info.trim.map(|t| t.source), Some(TrimSource::EditList));
    }

    #[test]
    fn no_gapless_information_and_garbage_are_none() {
        assert_eq!(info(file(None, &[(25, 1_024)], None)).trim, None);
        assert_eq!(info(vec![0xff; 64]).trim, None);
        assert_eq!(info(Vec::new()), Mp4Info::default());
    }
}
