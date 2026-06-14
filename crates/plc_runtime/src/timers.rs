//! IEC 61131-3 standard timer function blocks: TON, TOF, TP.
//!
//! Each timer is a stateful function block driven by deterministic virtual time
//! (milliseconds). Call `update(input, now_ms)` once per scan; read `q()` and
//! `et_ms()` for the boolean output and elapsed time. The preset time `pt_ms`
//! is fixed at construction.

/// On-delay timer: `Q` becomes true after `IN` is held true for `PT`.
#[derive(Debug, Clone)]
pub struct Ton {
    pt_ms: i64,
    start_ms: Option<i64>,
    q: bool,
    et_ms: i64,
}

impl Ton {
    pub fn new(pt_ms: i64) -> Self {
        Self {
            pt_ms,
            start_ms: None,
            q: false,
            et_ms: 0,
        }
    }

    pub fn update(&mut self, input: bool, now_ms: i64) -> bool {
        if input {
            let start = *self.start_ms.get_or_insert(now_ms);
            self.et_ms = (now_ms - start).clamp(0, self.pt_ms);
            self.q = self.et_ms >= self.pt_ms;
        } else {
            self.start_ms = None;
            self.et_ms = 0;
            self.q = false;
        }
        self.q
    }

    pub fn q(&self) -> bool {
        self.q
    }

    pub fn et_ms(&self) -> i64 {
        self.et_ms
    }
}

/// Off-delay timer: `Q` stays true for `PT` after `IN` falls.
#[derive(Debug, Clone)]
pub struct Tof {
    pt_ms: i64,
    off_start_ms: Option<i64>,
    q: bool,
    et_ms: i64,
}

impl Tof {
    pub fn new(pt_ms: i64) -> Self {
        Self {
            pt_ms,
            off_start_ms: None,
            q: false,
            et_ms: 0,
        }
    }

    pub fn update(&mut self, input: bool, now_ms: i64) -> bool {
        if input {
            self.off_start_ms = None;
            self.et_ms = 0;
            self.q = true;
        } else if let Some(off) = self.off_start_ms {
            self.et_ms = (now_ms - off).clamp(0, self.pt_ms);
            self.q = self.et_ms < self.pt_ms;
        } else if self.q {
            // Falling edge: begin the off-delay.
            self.off_start_ms = Some(now_ms);
            self.et_ms = 0;
            self.q = true;
        }
        self.q
    }

    pub fn q(&self) -> bool {
        self.q
    }

    pub fn et_ms(&self) -> i64 {
        self.et_ms
    }
}

/// Pulse timer: a rising edge on `IN` produces a `PT`-long pulse on `Q`.
#[derive(Debug, Clone)]
pub struct Tp {
    pt_ms: i64,
    pulse_start_ms: Option<i64>,
    prev_input: bool,
    q: bool,
    et_ms: i64,
}

impl Tp {
    pub fn new(pt_ms: i64) -> Self {
        Self {
            pt_ms,
            pulse_start_ms: None,
            prev_input: false,
            q: false,
            et_ms: 0,
        }
    }

    pub fn update(&mut self, input: bool, now_ms: i64) -> bool {
        // A pulse, once started, runs for the full preset regardless of input.
        if let Some(start) = self.pulse_start_ms {
            self.et_ms = (now_ms - start).clamp(0, self.pt_ms);
            if self.et_ms >= self.pt_ms {
                self.q = false;
                self.pulse_start_ms = None;
            } else {
                self.q = true;
            }
        }

        // Rising edge starts a new pulse when not already pulsing.
        if input && !self.prev_input && self.pulse_start_ms.is_none() && !self.q {
            self.pulse_start_ms = Some(now_ms);
            self.et_ms = 0;
            self.q = true;
        }

        self.prev_input = input;
        self.q
    }

    pub fn q(&self) -> bool {
        self.q
    }

    pub fn et_ms(&self) -> i64 {
        self.et_ms
    }
}
