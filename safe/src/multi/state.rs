#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum MultiState {
    Init,
    Pending,
    Connect,
    Resolving,
    Connecting,
    Tunneling,
    ProtoConnect,
    ProtoConnecting,
    Do,
    Doing,
    DoingMore,
    Did,
    Performing,
    RateLimiting,
    Done,
    Completed,
    MsgSent,
}

impl MultiState {
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::Init => "MSTATE_INIT",
            Self::Pending => "MSTATE_PENDING",
            Self::Connect => "MSTATE_CONNECT",
            Self::Resolving => "MSTATE_RESOLVING",
            Self::Connecting => "MSTATE_CONNECTING",
            Self::Tunneling => "MSTATE_TUNNELING",
            Self::ProtoConnect => "MSTATE_PROTOCONNECT",
            Self::ProtoConnecting => "MSTATE_PROTOCONNECTING",
            Self::Do => "MSTATE_DO",
            Self::Doing => "MSTATE_DOING",
            Self::DoingMore => "MSTATE_DOING_MORE",
            Self::Did => "MSTATE_DID",
            Self::Performing => "MSTATE_PERFORMING",
            Self::RateLimiting => "MSTATE_RATELIMITING",
            Self::Done => "MSTATE_DONE",
            Self::Completed => "MSTATE_COMPLETED",
            Self::MsgSent => "MSTATE_MSGSENT",
        }
    }

    pub(crate) const fn transition(_from: Self, to: Self) -> Self {
        to
    }
}
