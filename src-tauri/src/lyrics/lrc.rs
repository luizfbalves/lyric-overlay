#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Line {
    pub time_ms: u64,
    pub text: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Lyrics {
    pub lines: Vec<Line>,
}

impl Lyrics {
    /// Índice da última linha com `time_ms <= position_ms`; `None` antes da primeira.
    pub fn line_at(&self, position_ms: u64) -> Option<usize> {
        let idx = self.lines.partition_point(|l| l.time_ms <= position_ms);
        idx.checked_sub(1)
    }

    pub fn texts(&self) -> Vec<String> {
        self.lines.iter().map(|l| l.text.clone()).collect()
    }
}

fn digits(s: &str) -> bool {
    !s.is_empty() && s.chars().all(|c| c.is_ascii_digit())
}

fn parse_timestamp(tag: &str) -> Option<u64> {
    let (min, rest) = tag.split_once(':')?;
    if !digits(min) {
        return None;
    }
    let (sec, frac) = match rest.split_once('.') {
        Some((s, f)) => (s, Some(f)),
        None => (rest, None),
    };
    if !digits(sec) || sec.len() > 2 {
        return None;
    }
    let s: u64 = sec.parse().ok()?;
    if s >= 60 {
        return None;
    }
    let frac_ms = match frac {
        None => 0,
        Some(f) if digits(f) && f.len() <= 3 => {
            let v: u64 = f.parse().ok()?;
            v * 10u64.pow(3 - f.len() as u32)
        }
        Some(_) => return None,
    };
    let m: u64 = min.parse().ok()?;
    Some(m * 60_000 + s * 1000 + frac_ms)
}

pub fn parse_lrc(input: &str) -> Lyrics {
    let mut lines = Vec::new();
    for raw in input.lines() {
        let mut rest = raw;
        let mut times = Vec::new();
        loop {
            let t = rest.trim_start();
            if !t.starts_with('[') {
                break;
            }
            let Some(end) = t.find(']') else { break };
            match parse_timestamp(&t[1..end]) {
                Some(ms) => {
                    times.push(ms);
                    rest = &t[end + 1..];
                }
                None => break,
            }
        }
        if times.is_empty() {
            continue;
        }
        let text = rest.trim().to_string();
        for ms in times {
            lines.push(Line { time_ms: ms, text: text.clone() });
        }
    }
    lines.sort_by_key(|l| l.time_ms);
    Lyrics { lines }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_timestamp_formats() {
        let l = parse_lrc("[00:01.50]dois dígitos\n[00:02.250]três dígitos\n[00:03]sem fração\n[01:04.5]um dígito");
        let t: Vec<u64> = l.lines.iter().map(|x| x.time_ms).collect();
        assert_eq!(t, vec![1500, 2250, 3000, 64500]);
        assert_eq!(l.lines[0].text, "dois dígitos");
    }

    #[test]
    fn multiple_tags_on_one_line_are_sorted() {
        let l = parse_lrc("[00:10.00][00:02.00]refrão inventado\n[00:05.00]verso");
        let got: Vec<(u64, &str)> = l.lines.iter().map(|x| (x.time_ms, x.text.as_str())).collect();
        assert_eq!(got, vec![(2000, "refrão inventado"), (5000, "verso"), (10000, "refrão inventado")]);
    }

    #[test]
    fn ignores_metadata_and_garbage() {
        let l = parse_lrc("[ar:Banda Fictícia]\n[ti:Canção Teste]\n[offset:+100]\nlinha sem tag\n[xx:yy]lixo\n[00:01.00]ok");
        assert_eq!(l.lines.len(), 1);
        assert_eq!(l.lines[0].text, "ok");
    }

    #[test]
    fn keeps_empty_lines_and_handles_crlf() {
        let l = parse_lrc("[00:01.00]primeira\r\n[00:04.00]\r\n[00:06.00] terceira ");
        assert_eq!(l.lines.len(), 3);
        assert_eq!(l.lines[1].text, "");
        assert_eq!(l.lines[2].text, "terceira");
    }

    #[test]
    fn rejects_invalid_seconds_and_signs() {
        let l = parse_lrc("[00:75.00]segundos demais\n[+1:02.00]sinal\n[00:02.1234]fração longa\n[00:03.00]válida");
        assert_eq!(l.texts(), vec!["válida".to_string()]);
    }

    #[test]
    fn empty_input() {
        assert!(parse_lrc("").lines.is_empty());
    }

    #[test]
    fn line_at_boundaries() {
        let l = parse_lrc("[00:01.00]a\n[00:05.00]b\n[00:09.00]c");
        assert_eq!(l.line_at(0), None);
        assert_eq!(l.line_at(999), None);
        assert_eq!(l.line_at(1000), Some(0));
        assert_eq!(l.line_at(4999), Some(0));
        assert_eq!(l.line_at(5000), Some(1));
        assert_eq!(l.line_at(600_000), Some(2));
        assert_eq!(Lyrics::default().line_at(1000), None);
    }

    #[test]
    fn line_at_with_duplicate_timestamps_picks_last() {
        let l = parse_lrc("[00:02.00]x\n[00:02.00]y\n[00:04.00]z");
        assert_eq!(l.line_at(2500), Some(1));
    }
}
