//! A small project for the extension's manual and automated checks.
//!
//! It has a struct with methods, a free function that calls them, and a
//! recursive function, so the call hierarchy has something to show in both
//! directions.

pub struct Engine {
    name: String,
    power: u32,
}

impl Engine {
    pub fn new(name: &str, power: u32) -> Self {
        Engine {
            name: name.to_string(),
            power,
        }
    }

    pub fn start(&self) -> u32 {
        self.power.max(1)
    }

    pub fn stop(&self) {
        // Nothing to do; the fixture only needs a second method to call.
    }

    pub fn restart(&self) -> u32 {
        self.stop();
        self.start()
    }
}

pub fn build(name: &str) -> Engine {
    Engine::new(name, 10)
}

pub fn run() -> u32 {
    let engine = build("demo");
    engine.restart()
}

pub fn countdown(n: u32) -> u32 {
    if n == 0 {
        0
    } else {
        countdown(n - 1) + 1
    }
}
