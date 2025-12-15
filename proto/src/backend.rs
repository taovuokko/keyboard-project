use core::time::Duration;

/// Radio abstraction for sending/receiving framed packets.
pub trait RadioBackend {
    type Error;

    /// Transmit a frame. Implementation handles preamble/CRC as needed.
    fn transmit(&mut self, frame: &[u8]) -> Result<(), Self::Error>;

    /// Receive into `buf` with a timeout. Returns number of bytes received.
    fn receive(&mut self, buf: &mut [u8], timeout: Duration) -> Result<usize, Self::Error>;
}

/// Timer abstraction used for retries and wake timing.
pub trait TimerBackend {
    type Error;

    /// Monotonic time in milliseconds since boot.
    fn now_ms(&self) -> u64;

    /// Sleep/delay for the given duration.
    fn delay(&mut self, dur: Duration) -> Result<(), Self::Error>;
}

/// Entropy source for nonce/session salt generation.
pub trait EntropySource {
    type Error;

    /// Fill `buf` with random bytes.
    fn fill_bytes(&mut self, buf: &mut [u8]) -> Result<(), Self::Error>;
}
