//! What codec a video container carries, read from its bytes, offline.
//!
//! One responsibility: answer "will a browser screen be able to decode this?"
//! at the only moment the bytes are already in memory — import. FIELD-adjacent,
//! 2026-09-21: the one clip in a pilot church's library was an iPhone `.mov`
//! carrying `hvc1` (HEVC). The projector's own window decodes it; an OBS browser
//! source (Chromium without proprietary codecs) and a Windows WebView2 without
//! the HEVC extension paint nothing, and until F4 nothing said so. Relay does not
//! transcode; it warns where the cue is built and where the clip is chosen.
//!
//! This is a sniff, not a parser. It looks for the sample-entry four-character
//! codes ISO-BMFF/QuickTime files carry (`hvc1`, `hev1`, `avc1`, `av01`, `vp09`)
//! and the WebM/Matroska codec id strings. A file that names none of them is
//! reported as unknown, never as fine.

/// The codec family a container names, or `None` when the bytes say nothing
/// this sniff recognises.
///
/// `hevc` is the one that matters: it is the default an iPhone records in and
/// the one a browser screen most often cannot play. The others are reported so
/// the operator can see a known-good clip called known-good rather than merely
/// "not HEVC".
pub fn codec_hint(bytes: &[u8]) -> Option<&'static str> {
    // Cheap and bounded: a `moov` atom can sit at either end of a QuickTime
    // file, so both ends are scanned, and the middle (the `mdat` payload) is not.
    const WINDOW: usize = 8 * 1024 * 1024;
    let head = &bytes[..bytes.len().min(WINDOW)];
    let tail = &bytes[bytes.len().saturating_sub(WINDOW)..];
    let find = |needle: &[u8]| contains(head, needle) || contains(tail, needle);
    if find(b"hvc1") || find(b"hev1") || find(b"V_MPEGH/ISO/HEVC") {
        return Some("hevc");
    }
    if find(b"av01") || find(b"V_AV1") {
        return Some("av1");
    }
    if find(b"vp09") || find(b"V_VP9") {
        return Some("vp9");
    }
    if find(b"avc1") || find(b"avc3") || find(b"V_MPEG4/ISO/AVC") {
        return Some("h264");
    }
    None
}

fn contains(hay: &[u8], needle: &[u8]) -> bool {
    !needle.is_empty() && hay.windows(needle.len()).any(|w| w == needle)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn with_atom(atom: &[u8], where_: &str) -> Vec<u8> {
        let mut v = vec![0u8; 64];
        v.extend_from_slice(b"ftypqt  ");
        match where_ {
            "head" => {
                v.extend_from_slice(atom);
                v.extend(std::iter::repeat_n(0u8, 20 * 1024 * 1024));
            }
            _ => {
                v.extend(std::iter::repeat_n(0u8, 20 * 1024 * 1024));
                v.extend_from_slice(atom);
            }
        }
        v
    }

    #[test]
    fn an_iphone_clip_is_hevc_wherever_its_moov_sits() {
        assert_eq!(codec_hint(&with_atom(b"hvc1", "head")), Some("hevc"));
        assert_eq!(codec_hint(&with_atom(b"hvc1", "tail")), Some("hevc"));
        assert_eq!(codec_hint(&with_atom(b"hev1", "tail")), Some("hevc"));
    }

    #[test]
    fn h264_is_named_and_a_container_naming_nothing_is_unknown_not_fine() {
        assert_eq!(codec_hint(&with_atom(b"avc1", "tail")), Some("h264"));
        assert_eq!(codec_hint(b"\x00\x00\x00\x14ftypqt  nothing here"), None);
        assert_eq!(codec_hint(&[]), None);
    }

    #[test]
    fn hevc_beats_a_stray_avc_string_in_the_same_file() {
        // An HEVC file may carry `avc1` in a thumbnail track; HEVC is what the
        // main track is, and HEVC is the one that fails.
        let mut v = with_atom(b"avc1", "tail");
        v.extend_from_slice(b"hvc1");
        assert_eq!(codec_hint(&v), Some("hevc"));
    }
}
