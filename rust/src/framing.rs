pub const FRAME_LIMIT: usize = 65_536;

pub struct Decoder {
    bytes: Vec<u8>,
    peak: usize,
    failed: bool,
}

impl Decoder {
    pub fn new() -> Self {
        Self {
            bytes: Vec::with_capacity(FRAME_LIMIT),
            peak: 0,
            failed: false,
        }
    }

    pub fn peak_buffer_bytes(&self) -> usize {
        self.peak
    }

    pub fn feed<C, F>(&mut self, input: &[u8], mut clock: C, mut frame: F) -> Result<(), String>
    where
        C: FnMut() -> i64,
        F: FnMut(&[u8], i64) -> Result<(), String>,
    {
        if self.failed {
            return Err("decoder session already failed".into());
        }
        for &byte in input {
            if self.bytes.len() == FRAME_LIMIT {
                self.failed = true;
                return Err("logical frame exceeds limit".into());
            }
            self.bytes.push(byte);
            self.peak = self.peak.max(self.bytes.len());
            if byte == b'\n' {
                let receipt_qpc = clock();
                if std::str::from_utf8(&self.bytes).is_err() {
                    self.failed = true;
                    return Err("invalid frame UTF-8".into());
                }
                if let Err(error) = frame(&self.bytes, receipt_qpc) {
                    self.failed = true;
                    return Err(error);
                }
                self.bytes.clear();
            }
        }
        Ok(())
    }

    pub fn finish(&self) -> Result<(), String> {
        if self.failed || !self.bytes.is_empty() {
            Err("failed or incomplete NDJSON stream".into())
        } else {
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{Decoder, FRAME_LIMIT};

    #[test]
    fn split_multibyte_produces_exact_frames() {
        let mut decoder = Decoder::new();
        let mut calls = 0i64;
        let mut frames: Vec<Vec<u8>> = Vec::new();
        let first: &[u8] = b"{\"text\":\"\xc4";
        let second: &[u8] = b"\x80\"}\n{\"type\":\"complete\"}\n";
        decoder
            .feed(
                first,
                || {
                    calls += 1;
                    calls
                },
                |frame, _| {
                    frames.push(frame.to_vec());
                    Ok(())
                },
            )
            .unwrap();
        decoder
            .feed(
                second,
                || {
                    calls += 1;
                    calls
                },
                |frame, _| {
                    frames.push(frame.to_vec());
                    Ok(())
                },
            )
            .unwrap();
        decoder.finish().unwrap();
        assert_eq!(calls, 2);
        assert_eq!(frames.len(), 2);
        assert_eq!(frames[0], b"{\"text\":\"\xc4\x80\"}\n");
        assert_eq!(frames[1], b"{\"type\":\"complete\"}\n");
    }

    #[test]
    fn maximum_frame_accepted_at_limit() {
        let mut decoder = Decoder::new();
        let mut frames = 0usize;
        let payload = vec![b'x'; FRAME_LIMIT - 1];
        decoder
            .feed(
                &payload,
                || 0,
                |_, _| {
                    frames += 1;
                    Ok(())
                },
            )
            .unwrap();
        decoder
            .feed(
                b"\n",
                || 0,
                |_, _| {
                    frames += 1;
                    Ok(())
                },
            )
            .unwrap();
        assert_eq!(decoder.peak_buffer_bytes(), FRAME_LIMIT);
        assert_eq!(frames, 1);
    }

    #[test]
    fn oversized_rejected_before_growth() {
        let mut decoder = Decoder::new();
        let mut frames = 0usize;
        let payload = vec![b'x'; FRAME_LIMIT + 1];
        let result = decoder.feed(
            &payload,
            || 0,
            |_, _| {
                frames += 1;
                Ok(())
            },
        );
        assert!(result.is_err());
        assert_eq!(decoder.peak_buffer_bytes(), FRAME_LIMIT);
        assert_eq!(frames, 0);
        assert!(decoder.feed(b"\n", || 0, |_, _| Ok(())).is_err());
        assert!(decoder.finish().is_err());
    }

    #[test]
    fn partial_frame_fails_finish() {
        let mut decoder = Decoder::new();
        decoder.feed(b"{\"partial\":", || 0, |_, _| Ok(())).unwrap();
        assert!(decoder.finish().is_err());
    }

    #[test]
    fn invalid_utf8_rejected_only_at_frame_end() {
        let mut decoder = Decoder::new();
        let mut frames = 0usize;
        decoder
            .feed(
                b"{\"bad\":\"\xff",
                || 0,
                |_, _| {
                    frames += 1;
                    Ok(())
                },
            )
            .unwrap();
        assert!(
            decoder
                .feed(
                    b"\"}\n",
                    || 0,
                    |_, _| {
                        frames += 1;
                        Ok(())
                    }
                )
                .is_err()
        );
        assert_eq!(frames, 0);
    }
}
