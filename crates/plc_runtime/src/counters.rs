//! IEC 61131-3 counter function blocks: CTU, CTD, CTUD.
//!
//! Each counter is a stateful function block. Count inputs are edge-triggered;
//! call `update(...)` once per scan and read `q()` / `cv()`.

/// Up counter. `Q` is true when `CV >= PV`.
#[derive(Debug, Clone)]
pub struct Ctu {
    cv: i64,
    prev_cu: bool,
    q: bool,
}

impl Default for Ctu {
    fn default() -> Self {
        Self::new()
    }
}

impl Ctu {
    pub fn new() -> Self {
        Self {
            cv: 0,
            prev_cu: false,
            q: false,
        }
    }

    pub fn update(&mut self, cu: bool, reset: bool, pv: i64) -> bool {
        if reset {
            self.cv = 0;
        } else if cu && !self.prev_cu {
            self.cv = self.cv.saturating_add(1);
        }
        self.prev_cu = cu;
        self.q = self.cv >= pv;
        self.q
    }

    pub fn q(&self) -> bool {
        self.q
    }

    pub fn cv(&self) -> i64 {
        self.cv
    }
}

/// Down counter. `Q` is true when `CV <= 0`.
#[derive(Debug, Clone)]
pub struct Ctd {
    cv: i64,
    prev_cd: bool,
    q: bool,
}

impl Default for Ctd {
    fn default() -> Self {
        Self::new()
    }
}

impl Ctd {
    pub fn new() -> Self {
        Self {
            cv: 0,
            prev_cd: false,
            q: false,
        }
    }

    pub fn update(&mut self, cd: bool, load: bool, pv: i64) -> bool {
        if load {
            self.cv = pv;
        } else if cd && !self.prev_cd {
            self.cv = self.cv.saturating_sub(1);
        }
        self.prev_cd = cd;
        self.q = self.cv <= 0;
        self.q
    }

    pub fn q(&self) -> bool {
        self.q
    }

    pub fn cv(&self) -> i64 {
        self.cv
    }
}

/// Up/down counter. `QU` is true when `CV >= PV`; `QD` when `CV <= 0`.
#[derive(Debug, Clone)]
pub struct Ctud {
    cv: i64,
    prev_cu: bool,
    prev_cd: bool,
    qu: bool,
    qd: bool,
}

impl Default for Ctud {
    fn default() -> Self {
        Self::new()
    }
}

impl Ctud {
    pub fn new() -> Self {
        Self {
            cv: 0,
            prev_cu: false,
            prev_cd: false,
            qu: false,
            qd: true,
        }
    }

    pub fn update(&mut self, cu: bool, cd: bool, reset: bool, load: bool, pv: i64) {
        if reset {
            self.cv = 0;
        } else if load {
            self.cv = pv;
        } else {
            if cu && !self.prev_cu {
                self.cv = self.cv.saturating_add(1);
            }
            if cd && !self.prev_cd {
                self.cv = self.cv.saturating_sub(1);
            }
        }
        self.prev_cu = cu;
        self.prev_cd = cd;
        self.qu = self.cv >= pv;
        self.qd = self.cv <= 0;
    }

    pub fn qu(&self) -> bool {
        self.qu
    }

    pub fn qd(&self) -> bool {
        self.qd
    }

    pub fn cv(&self) -> i64 {
        self.cv
    }
}
