//! IEC 61131-3 edge detector function blocks: R_TRIG, F_TRIG.
//!
//! Each detector latches the previous clock value and reports a one-scan pulse
//! on the corresponding edge. Call `update(clk)` once per scan.

/// Rising-edge detector: `Q` is true for one scan when `CLK` goes false→true.
#[derive(Debug, Clone, Default)]
pub struct RTrig {
    prev: bool,
    q: bool,
}

impl RTrig {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn update(&mut self, clk: bool) -> bool {
        self.q = clk && !self.prev;
        self.prev = clk;
        self.q
    }

    pub fn q(&self) -> bool {
        self.q
    }
}

/// Falling-edge detector: `Q` is true for one scan when `CLK` goes true→false.
#[derive(Debug, Clone, Default)]
pub struct FTrig {
    prev: bool,
    q: bool,
}

impl FTrig {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn update(&mut self, clk: bool) -> bool {
        self.q = !clk && self.prev;
        self.prev = clk;
        self.q
    }

    pub fn q(&self) -> bool {
        self.q
    }
}
