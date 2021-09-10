use crate::{error::Error,
            util};
use clap::ArgMatches;
use native_tls::Certificate;
use std::{collections::HashMap,
          fmt,
          fs,
          io,
          net::{IpAddr,
                Ipv4Addr,
                SocketAddr,
                SocketAddrV4,
                ToSocketAddrs},
          num::ParseIntError,
          ops::{Deref,
                DerefMut},
          option,
          path::PathBuf,
          result,
          str::FromStr,
          time::Duration};

/// Bundles up information about the user and group that a supervised
/// service should be run as. If the Supervisor itself is running with
/// root-like permissions, then these will be for `SVC_USER` and
/// `SVC_GROUP` for a service. If not, it will be for the user the
/// Supervisor itself is running as.
///
/// On Windows, all but `username` will be `None`. On Linux,
/// `username` and `groupname` may legitimately be `None`, but `uid`
/// and `gid` should always be `Some`.
#[derive(Debug, Default)]
pub struct UserInfo {
    /// Windows required, Linux optional
    pub username:  Option<String>,
    /// Linux preferred
    pub uid:       Option<u32>,
    /// Linux optional
    pub groupname: Option<String>,
    /// Linux preferred
    pub gid:       Option<u32>,
}

#[derive(Clone, Deserialize, Serialize)]
// TODO (DM): This is unnecessarily difficult due to this issue in serde
// https://github.com/serde-rs/serde/issues/723. The easiest way to get around the issue is to use
// these proxy types.
#[serde(try_from = "&str", into = "String")]
pub struct EventStreamMetaPair(String, String);

impl FromStr for EventStreamMetaPair {
    type Err = io::Error;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s.split('=').collect::<Vec<_>>().as_slice() {
            [key, value] if !key.is_empty() && !value.is_empty() => {
                Ok(Self(String::from(*key), String::from(*value)))
            }
            _ => {
                let e = format!("Invalid key-value pair given (must be '='-delimited pair of \
                                 non-empty strings): {}",
                                s);
                Err(io::Error::new(io::ErrorKind::InvalidInput, e))
            }
        }
    }
}

impl fmt::Display for EventStreamMetaPair {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result { write!(f, "{}={} ", self.0, self.1) }
}

impl std::convert::TryFrom<&str> for EventStreamMetaPair {
    type Error = Error;

    fn try_from(s: &str) -> Result<Self, Self::Error> { Ok(EventStreamMetaPair::from_str(s)?) }
}

#[allow(clippy::from_over_into)]
impl Into<String> for EventStreamMetaPair {
    fn into(self) -> String { self.to_string() }
}

/// Captures arbitrary key-value pair metadata to attach to all events
/// generated by the Supervisor.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct EventStreamMetadata(HashMap<String, String>);

#[allow(clippy::from_over_into)]
impl Into<HashMap<String, String>> for EventStreamMetadata {
    fn into(self) -> HashMap<String, String> { self.0 }
}

impl From<HashMap<String, String>> for EventStreamMetadata {
    fn from(map: HashMap<String, String>) -> Self { Self(map) }
}

impl From<Vec<EventStreamMetaPair>> for EventStreamMetadata {
    fn from(pairs: Vec<EventStreamMetaPair>) -> Self {
        pairs.into_iter()
             .map(|p| (p.0, p.1))
             .collect::<HashMap<_, _>>()
             .into()
    }
}

impl EventStreamMetadata {
    /// The name of the Clap argument we'll use for arguments of this type.
    pub const ARG_NAME: &'static str = "EVENT_META";
}

/// This represents an environment variable that holds an authentication token which enables
/// integration with Automate. Supervisors use this token to connect to the messaging server
/// on the Automate side in order to send data about the services they're running via event
/// messages. If the environment variable is present, its value is the auth token. If it's not
/// present and the feature flag for the Event Stream is enabled, initialization of the Event
/// Stream will fail.
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
pub struct EventStreamToken(String);

impl EventStreamToken {
    /// The name of the Clap argument we'll use for arguments of this type.
    pub const ARG_NAME: &'static str = "EVENT_STREAM_TOKEN";
    // Ideally, we'd like to take advantage of
    // `habitat_core::env::Config` trait, but that currently requires
    // a `Default` implementation, and there isn't really a legitimate
    // default value right now.
    pub const ENVVAR: &'static str = "HAB_AUTOMATE_AUTH_TOKEN";
}

impl EventStreamToken {
    /// Ensure that user input from Clap can be converted an instance
    /// of a token.
    #[allow(clippy::needless_pass_by_value)] // Signature required by CLAP
    pub fn validate(value: String) -> result::Result<(), String> {
        value.parse::<Self>().map(|_| ()).map_err(|e| e.to_string())
    }
}

impl<'a> From<&'a ArgMatches<'a>> for EventStreamToken {
    /// Create an instance of `EventStreamToken` from validated
    /// user input.
    fn from(m: &ArgMatches) -> Self {
        m.value_of(Self::ARG_NAME)
         .expect("HAB_AUTOMATE_AUTH_TOKEN should be set")
         .parse()
         .expect("HAB_AUTOMATE_AUTH_TOKEN should be validated at this point")
    }
}

impl FromStr for EventStreamToken {
    type Err = Error;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        if s.is_empty() {
            Err(Error::InvalidEventStreamToken(s.to_string()))
        } else {
            Ok(EventStreamToken(s.to_string()))
        }
    }
}

impl fmt::Display for EventStreamToken {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result { write!(f, "{}", self.0) }
}

/// The event stream connection method.
#[derive(Clone, Copy, Debug, PartialEq, Deserialize, Serialize)]
#[serde(from = "u64", into = "u64")]
pub enum EventStreamConnectMethod {
    /// Immediately start the supervisor regardless of the event stream status.
    Immediate,
    /// Before starting the supervisor, wait the timeout for an event stream connection.
    /// If after the timeout there is no connection, exit the supervisor.
    Timeout { secs: u64 },
}

impl EventStreamConnectMethod {
    /// The name of the Clap argument.
    pub const ARG_NAME: &'static str = "EVENT_STREAM_CONNECT_TIMEOUT";
    /// The environment variable to set this value.
    pub const ENVVAR: &'static str = "HAB_EVENT_STREAM_CONNECT_TIMEOUT";
}

impl FromStr for EventStreamConnectMethod {
    type Err = ParseIntError;

    fn from_str(s: &str) -> ::std::result::Result<Self, Self::Err> {
        let secs = s.parse::<u64>()?;
        Ok(Self::from(secs))
    }
}

impl From<u64> for EventStreamConnectMethod {
    /// A numeric value of zero indicates there is no timeout and the `Immediate` variant is
    /// returned. Otherwise, the `Timeout` variant is returned.
    fn from(secs: u64) -> Self {
        if secs == 0 {
            EventStreamConnectMethod::Immediate
        } else {
            EventStreamConnectMethod::Timeout { secs }
        }
    }
}

impl From<EventStreamConnectMethod> for u64 {
    fn from(connect_method: EventStreamConnectMethod) -> Self {
        match connect_method {
            EventStreamConnectMethod::Immediate => 0,
            EventStreamConnectMethod::Timeout { secs } => secs,
        }
    }
}

impl fmt::Display for EventStreamConnectMethod {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result { write!(f, "{}", u64::from(*self)) }
}

#[allow(clippy::from_over_into)]
impl Into<Option<Duration>> for EventStreamConnectMethod {
    fn into(self) -> Option<Duration> {
        match self {
            EventStreamConnectMethod::Immediate => None,
            EventStreamConnectMethod::Timeout { secs } => Some(Duration::from_secs(secs)),
        }
    }
}

#[derive(Clone, Deserialize, Serialize)]
// TODO (DM): This is unnecessarily difficult due to this issue in serde
// https://github.com/serde-rs/serde/issues/723. The easiest way to get around the issue is to use
// these proxy types.
#[serde(try_from = "&str", into = "PathBuf")]
pub struct EventStreamServerCertificate {
    path:        PathBuf,
    certificate: Certificate,
}

impl EventStreamServerCertificate {
    /// The name of the Clap argument.
    pub const ARG_NAME: &'static str = "EVENT_STREAM_SERVER_CERTIFICATE";

    #[allow(clippy::needless_pass_by_value)] // Signature required by CLAP
    pub fn validate(value: String) -> result::Result<(), String> {
        value.parse::<Self>().map(|_| ()).map_err(|e| e.to_string())
    }

    /// Create an instance of `EventStreamServerCertificate` from validated user input.
    pub fn from_arg_matches(m: &ArgMatches) -> Option<Self> {
        m.value_of(Self::ARG_NAME).map(|value| {
                                      value.parse().expect("EVENT_STREAM_SERVER_CERTIFICATE \
                                                            should be validated")
                                  })
    }
}

impl FromStr for EventStreamServerCertificate {
    type Err = Error;

    /// Treat the string as a file path. Try and read the file as a PEM certificate.
    fn from_str(s: &str) -> ::std::result::Result<Self, Self::Err> {
        let path = PathBuf::from_str(s).expect("Infallible conversion");
        let contents = fs::read(&path)?;
        Ok(EventStreamServerCertificate { path,
                                          certificate: Certificate::from_pem(&contents)? })
    }
}

impl std::convert::TryFrom<&str> for EventStreamServerCertificate {
    type Error = Error;

    fn try_from(s: &str) -> Result<Self, Self::Error> { EventStreamServerCertificate::from_str(s) }
}

#[allow(clippy::from_over_into)]
impl Into<Certificate> for EventStreamServerCertificate {
    fn into(self) -> Certificate { self.certificate }
}

#[allow(clippy::from_over_into)]
impl Into<PathBuf> for EventStreamServerCertificate {
    fn into(self) -> PathBuf { self.path }
}

#[allow(clippy::from_over_into)]
impl Into<PathBuf> for &EventStreamServerCertificate {
    fn into(self) -> PathBuf { self.path.clone() }
}

impl fmt::Debug for EventStreamServerCertificate {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f,
               "EventStreamServerCertificate {{ path: {:?} }}",
               self.path)
    }
}

impl fmt::Display for EventStreamServerCertificate {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.path.to_string_lossy())
    }
}

// This impl is only use for testing. We cannot annotate it with `#[test]` because the tests are in
// a different crate.
impl PartialEq<EventStreamServerCertificate> for EventStreamServerCertificate {
    fn eq(&self, other: &EventStreamServerCertificate) -> bool {
        match (self.certificate.to_der(), other.certificate.to_der()) {
            (Ok(s), Ok(o)) => s == o,
            _ => false,
        }
    }
}

/// A wrapper around `ListenCtlAddr` that keeps track of the domain the socket address was resolved
/// from. Ideally this would be done by `env_config_socketaddr`.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(try_from = "&str", into = "String")]
pub struct ResolvedListenCtlAddr {
    domain: String,
    addr:   ListenCtlAddr,
}

impl FromStr for ResolvedListenCtlAddr {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let (domain, addr) =
            util::resolve_socket_addr_with_default_port(s, ListenCtlAddr::DEFAULT_PORT)?;
        Ok(Self { domain,
                  addr: ListenCtlAddr::from(addr) })
    }
}

impl std::fmt::Display for ResolvedListenCtlAddr {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{}:{}", self.domain, self.addr.port())
    }
}

impl Default for ResolvedListenCtlAddr {
    fn default() -> Self { ListenCtlAddr::default_as_str().parse().unwrap() }
}

impl ResolvedListenCtlAddr {
    pub fn addr(&self) -> SocketAddr { self.addr.into() }

    pub fn domain(&self) -> &str { &self.domain }
}

impl From<ResolvedListenCtlAddr> for ListenCtlAddr {
    fn from(r: ResolvedListenCtlAddr) -> Self { r.addr }
}

habitat_core::impl_try_from_str_and_into_string!(ResolvedListenCtlAddr);

habitat_core::env_config_socketaddr!(#[derive(Clone, Copy, PartialEq, Eq, Debug, Deserialize, Serialize)]
                                     pub GossipListenAddr,
                                     HAB_LISTEN_GOSSIP,
                                     0, 0, 0, 0, Self::DEFAULT_PORT);

impl GossipListenAddr {
    pub const DEFAULT_PORT: u16 = 9638;

    /// When local gossip mode is used we still startup the gossip layer but set
    /// it to listen on 127.0.0.2 to scope it to the local node but ignore connections from
    /// 127.0.0.1. This turns out to be much simpler than the cascade of changes that would
    /// be involved in not setting up a gossip listener at all.
    pub fn local_only() -> Self {
        GossipListenAddr(SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::new(127, 0, 0, 2),
                                                          Self::DEFAULT_PORT)))
    }

    /// Generate an address at which a server configured with this
    /// GossipListenAddr can communicate with itself.
    ///
    /// In particular, a server configured to listen on `0.0.0.0` vs
    /// `192.168.1.1` should be contacted via `127.0.0.1` in the
    /// former case, but `192.168.1.1` in the latter.
    pub fn local_addr(&self) -> Self {
        let mut addr = *self;
        if addr.0.ip().is_unspecified() {
            // TODO (CM): Support IPV6, once we do that more broadly
            addr.0.set_ip(IpAddr::V4(Ipv4Addr::LOCALHOST));
        }
        addr
    }
}

impl Deref for GossipListenAddr {
    type Target = SocketAddr;

    fn deref(&self) -> &SocketAddr { &self.0 }
}

impl DerefMut for GossipListenAddr {
    fn deref_mut(&mut self) -> &mut SocketAddr { &mut self.0 }
}

impl ToSocketAddrs for GossipListenAddr {
    type Iter = option::IntoIter<SocketAddr>;

    fn to_socket_addrs(&self) -> io::Result<Self::Iter> { self.0.to_socket_addrs() }
}

habitat_core::env_config_socketaddr!(#[derive(PartialEq, Eq, Debug, Clone, Copy, Deserialize, Serialize)]
                                     pub HttpListenAddr,
                                     HAB_LISTEN_HTTP,
                                     0, 0, 0, 0, 9631);
impl HttpListenAddr {
    pub fn new(ip: IpAddr, port: u16) -> Self { Self(SocketAddr::new(ip, port)) }
}

impl Deref for HttpListenAddr {
    type Target = SocketAddr;

    fn deref(&self) -> &SocketAddr { &self.0 }
}

impl DerefMut for HttpListenAddr {
    fn deref_mut(&mut self) -> &mut SocketAddr { &mut self.0 }
}

impl ToSocketAddrs for HttpListenAddr {
    type Iter = option::IntoIter<SocketAddr>;

    fn to_socket_addrs(&self) -> io::Result<Self::Iter> { self.0.to_socket_addrs() }
}

habitat_core::env_config_socketaddr!(#[derive(Clone, Copy, PartialEq, Eq, Debug, Deserialize, Serialize)]
                                     pub ListenCtlAddr,
                                     HAB_LISTEN_CTL,
                                     Ipv4Addr::LOCALHOST, Self::DEFAULT_PORT);

impl ListenCtlAddr {
    pub const DEFAULT_PORT: u16 = 9632;

    pub fn new(ip: Ipv4Addr, port: u16) -> Self {
        ListenCtlAddr(SocketAddr::V4(SocketAddrV4::new(ip, port)))
    }

    pub fn ip(&self) -> IpAddr { self.0.ip() }

    pub fn port(&self) -> u16 { self.0.port() }
}

impl AsRef<SocketAddr> for ListenCtlAddr {
    fn as_ref(&self) -> &SocketAddr { &self.0 }
}

#[cfg(test)]
mod tests {
    use super::*;

    mod auth_token {
        use super::*;

        #[test]
        fn cannot_parse_from_empty_string() { assert!("".parse::<EventStreamToken>().is_err()) }
    }

    mod gossip_listen_addr {
        use super::*;
        #[test]
        fn local_addr_for_gossip_listen_addr_works_for_unspecified_address() {
            let listen_addr = GossipListenAddr::default();
            assert!(listen_addr.0.ip().is_unspecified());

            let local_addr = listen_addr.local_addr();
            assert!(local_addr.0.ip().is_loopback());
        }

        #[test]
        fn local_addr_for_gossip_listen_addr_returns_same_ip_for_a_specified_address() {
            let mut listen_addr = GossipListenAddr::default();
            listen_addr.0
                       .set_ip(IpAddr::V4(Ipv4Addr::new(192, 168, 1, 1)));
            assert!(!listen_addr.0.ip().is_loopback());

            let local_addr = listen_addr.local_addr();
            assert_eq!(listen_addr, local_addr);
        }
    }

    mod env_config {
        use habitat_core::{env::Config as EnvConfig,
                           locked_env_var};
        use std::{env,
                  num::ParseIntError,
                  result,
                  str::FromStr};

        #[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Eq)]
        struct Thingie(u64);

        impl Default for Thingie {
            fn default() -> Self { Thingie(2112) }
        }

        impl FromStr for Thingie {
            type Err = ParseIntError;

            fn from_str(s: &str) -> result::Result<Self, Self::Err> {
                let raw = s.parse::<u64>()?;
                Ok(Thingie(raw))
            }
        }

        locked_env_var!(HAB_TESTING_THINGIE, lock_hab_testing_thingie);

        impl EnvConfig for Thingie {
            const ENVVAR: &'static str = "HAB_TESTING_THINGIE";
        }

        #[test]
        fn no_env_var_yields_default() {
            let _envvar = lock_hab_testing_thingie();
            assert!(env::var("HAB_TESTING_THINGIE").is_err()); // it's not set
            assert_eq!(Thingie::configured_value(), Thingie(2112));
            assert_eq!(Thingie::configured_value(), Thingie::default());
        }

        #[test]
        fn parsable_env_var_yields_parsed_value() {
            let envvar = lock_hab_testing_thingie();
            envvar.set("123");
            assert_eq!(Thingie::configured_value(), Thingie(123));
            assert_ne!(Thingie::configured_value(), Thingie::default());
        }

        #[test]
        fn unparsable_env_var_yields_default() {
            let envvar = lock_hab_testing_thingie();
            envvar.set("I'm not a number");
            assert_eq!(Thingie::configured_value(), Thingie::default());
        }
    }
}
