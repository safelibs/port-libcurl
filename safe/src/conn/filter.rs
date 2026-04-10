pub(crate) trait ConnectionFilter {
    fn name(&self) -> &'static str;
    fn is_terminal(&self) -> bool {
        false
    }
}

#[derive(Default)]
pub(crate) struct ConnectionFilterChain {
    filters: Vec<Box<dyn ConnectionFilter + Send + Sync>>,
}

impl ConnectionFilterChain {
    pub(crate) fn push(&mut self, filter: Box<dyn ConnectionFilter + Send + Sync>) {
        self.filters.push(filter);
    }

    pub(crate) fn names(&self) -> Vec<&'static str> {
        self.filters.iter().map(|filter| filter.name()).collect()
    }
}
