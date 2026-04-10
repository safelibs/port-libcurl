#[derive(Clone, Debug, Default)]
pub(crate) struct AltSvcCache {
    pub enabled: bool,
    pub path: Option<String>,
    pub ctrl_bits: i64,
    entries: Vec<String>,
}

impl AltSvcCache {
    pub(crate) fn clear_runtime(&mut self) {
        self.entries.clear();
    }

    pub(crate) fn remember_raw(&mut self, line: &str) {
        self.entries.push(line.to_string());
    }
}
