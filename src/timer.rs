use web_time::Instant;
#[allow(dead_code)]
pub struct Timer(String, Instant);

#[allow(dead_code)]
impl Timer {
    pub fn now(name: String) -> Self {
        Self(name, Instant::now())
    }

    pub fn print(self) {
        log::info!("{}: {}ms", self.0, (Instant::now() - self.1).as_millis());
    }
}
