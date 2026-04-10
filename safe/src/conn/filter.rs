use core::ffi::c_long;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum ConnectionFilterStep {
    ResolveOverrides {
        count: usize,
    },
    ConnectTo {
        target: String,
    },
    Proxy {
        authority: String,
        tunnel: bool,
    },
    ShareLock {
        scope: String,
    },
    LowSpeedGuard {
        limit_bytes_per_second: c_long,
        time_window_secs: c_long,
    },
    ConnectOnly,
    FollowRedirects,
    TransferLoop,
}

impl ConnectionFilterStep {
    pub(crate) const fn name(&self) -> &'static str {
        match self {
            Self::ResolveOverrides { .. } => "resolve-overrides",
            Self::ConnectTo { .. } => "connect-to",
            Self::Proxy { .. } => "proxy",
            Self::ShareLock { .. } => "share-lock",
            Self::LowSpeedGuard { .. } => "low-speed-guard",
            Self::ConnectOnly => "connect-only",
            Self::FollowRedirects => "follow-location",
            Self::TransferLoop => "transfer-loop",
        }
    }

    pub(crate) const fn is_terminal(&self) -> bool {
        matches!(self, Self::TransferLoop)
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct ConnectionFilterChain {
    filters: Vec<ConnectionFilterStep>,
}

impl ConnectionFilterChain {
    pub(crate) fn push(&mut self, filter: ConnectionFilterStep) {
        self.filters.push(filter);
    }

    pub(crate) fn iter(&self) -> impl Iterator<Item = &ConnectionFilterStep> {
        self.filters.iter()
    }

    pub(crate) fn names(&self) -> Vec<&'static str> {
        self.filters
            .iter()
            .map(ConnectionFilterStep::name)
            .collect()
    }

    pub(crate) fn terminal(&self) -> Option<&ConnectionFilterStep> {
        self.filters.iter().find(|filter| filter.is_terminal())
    }
}
