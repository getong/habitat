use pin_project::pin_project;
use rustls::{pki_types::ServerName,
             ClientConfig as TlsClientConfig,
             ServerConfig as TlsServerConfig};
use std::{convert::TryFrom,
          pin::Pin,
          sync::Arc,
          task::{Context,
                 Poll}};
use tokio::{io::{self,
                 AsyncRead,
                 AsyncWrite,
                 ReadBuf},
            net::TcpStream};
use tokio_rustls::{TlsAcceptor,
                   TlsConnector,
                   TlsStream};

/// A wrapper type that can either be a raw TCP stream or a TCP stream with TLS.
#[pin_project(project = TcpOrTlsStreamProj)]
#[allow(clippy::large_enum_variant)]
pub enum TcpOrTlsStream {
    TcpStream(#[pin] TcpStream),
    TlsStream(#[pin] TlsStream<TcpStream>),
}

impl TcpOrTlsStream {
    /// Create a new `TcpStream`
    pub fn new(stream: TcpStream) -> Self { Self::TcpStream(stream) }

    /// Create a new `TlsStream` using server configuration
    pub async fn new_tls_server(stream: TcpStream,
                                tls_config: Arc<TlsServerConfig>)
                                -> Result<Self, (io::Error, TcpStream)> {
        let tcp_stream = Self::new(stream);
        tcp_stream.maybe_upgrade_to_tls_server(tls_config).await
    }

    /// Create a new `TlsStream` using client configuration
    pub async fn new_tls_client(stream: TcpStream,
                                tls_config: Arc<TlsClientConfig>,
                                domain: &str)
                                -> Result<Self, (io::Error, TcpStream)> {
        let tcp_stream = Self::new(stream);
        tcp_stream.maybe_upgrade_to_tls_client(tls_config, domain)
                  .await
    }

    /// Upgrade a `TcpStream` into a `TlsStream` using server configuration
    async fn maybe_upgrade_to_tls_server(self,
                                         tls_config: Arc<TlsServerConfig>)
                                         -> Result<Self, (io::Error, TcpStream)> {
        let tls_server_stream = match self {
            Self::TcpStream(stream) => {
                let tls_acceptor = TlsAcceptor::from(tls_config);
                let tls_stream = tls_acceptor.accept(stream).into_fallible().await?;
                Self::TlsStream(TlsStream::Server(tls_stream))
            }
            stream @ Self::TlsStream(_) => stream,
        };
        Ok(tls_server_stream)
    }

    /// Upgrade a `TcpStream` to a `TlsStream` using client configuration
    async fn maybe_upgrade_to_tls_client(self,
                                         tls_config: Arc<TlsClientConfig>,
                                         domain: &str)
                                         -> Result<Self, (io::Error, TcpStream)> {
        let tls_client_stream = match self {
            Self::TcpStream(stream) => {
                let tls_connector = TlsConnector::from(tls_config);
                let server_name = match ServerName::try_from(domain.to_string()) {
                    Ok(name) => name,
                    Err(_) => {
                        let error = io::Error::new(io::ErrorKind::InvalidInput,
                                                   format!("invalid DNS name '{}'", domain));
                        return Err((error, stream));
                    }
                };
                let tls_stream = tls_connector.connect(server_name, stream)
                                              .into_fallible()
                                              .await?;
                Self::TlsStream(TlsStream::Client(tls_stream))
            }
            stream @ Self::TlsStream(_) => stream,
        };
        Ok(tls_client_stream)
    }
}

impl AsyncRead for TcpOrTlsStream {
    fn poll_read(self: Pin<&mut Self>,
                 cx: &mut Context,
                 buf: &mut ReadBuf<'_>)
                 -> Poll<io::Result<()>> {
        match self.project() {
            TcpOrTlsStreamProj::TcpStream(stream) => stream.poll_read(cx, buf),
            TcpOrTlsStreamProj::TlsStream(stream) => {
                match stream.poll_read(cx, buf) {
                    // As of rustls 0.20.0, a UnexpectedEof is returned when reading from
                    // a connection that has not been cleanly closed. The supervisor now
                    // sends a close_notify by calling poll_shutdown on the stream when
                    // a message is sent from the ctl-gateway. While that avoids the
                    // UnexpectedEof, it is possible that a supervisor client in a hab cli
                    // using rustls > 0.20.0 will be communicating with a < 0.20.0 supervisor.
                    // For that case, we check here for a UnexpectedEof and return
                    // Poll::Ready(Ok(())) to mimic the 0.19 rustls behavior. We should
                    // consider removing this when releasing a habitat 2.0.
                    Poll::Ready(Err(err)) if err.kind() == io::ErrorKind::UnexpectedEof => {
                        Poll::Ready(Ok(()))
                    }
                    output => output,
                }
            }
        }
    }
}

impl AsyncWrite for TcpOrTlsStream {
    fn poll_write(self: Pin<&mut Self>, cx: &mut Context, buf: &[u8]) -> Poll<io::Result<usize>> {
        match self.project() {
            TcpOrTlsStreamProj::TcpStream(stream) => stream.poll_write(cx, buf),
            TcpOrTlsStreamProj::TlsStream(stream) => stream.poll_write(cx, buf),
        }
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context) -> Poll<io::Result<()>> {
        match self.project() {
            TcpOrTlsStreamProj::TcpStream(stream) => stream.poll_flush(cx),
            TcpOrTlsStreamProj::TlsStream(stream) => stream.poll_flush(cx),
        }
    }

    fn poll_shutdown(self: Pin<&mut Self>, cx: &mut Context) -> Poll<io::Result<()>> {
        match self.project() {
            TcpOrTlsStreamProj::TcpStream(stream) => stream.poll_shutdown(cx),
            TcpOrTlsStreamProj::TlsStream(stream) => stream.poll_shutdown(cx),
        }
    }
}
