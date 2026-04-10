#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ResolverOwner {
    Easy,
    Multi,
    Share,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct ResolverLease {
    pub owner: ResolverOwner,
    pub shared: bool,
}

impl ResolverLease {
    pub(crate) const fn shared(owner: ResolverOwner) -> Self {
        Self {
            owner,
            shared: true,
        }
    }

    pub(crate) const fn exclusive(owner: ResolverOwner) -> Self {
        Self {
            owner,
            shared: false,
        }
    }
}
