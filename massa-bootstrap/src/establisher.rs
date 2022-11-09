// Copyright (c) 2022 MASSA LABS <info@massa.net>

#[cfg(test)]
pub mod types {
    pub type Duplex = crate::tests::mock_establisher::Duplex;

    pub type Listener = crate::tests::mock_establisher::MockListener;

    pub type Connector = crate::tests::mock_establisher::MockConnector;

    pub type Establisher = crate::tests::mock_establisher::MockEstablisher;
}

#[cfg(not(test))]
/// Connection types
pub mod types {
    use crate::tools::normalize_ip;
    use massa_time::MassaTime;
    use std::{
        collections::HashSet,
        io,
        net::{IpAddr, SocketAddr},
    };
    use tokio::{
        net::{TcpListener, TcpStream},
        time::timeout,
    };
    /// duplex connection
    pub type Duplex = TcpStream;
    /// listener
    pub type Listener = DefaultListener;
    /// connector
    pub type Connector = DefaultConnector;
    /// connection establisher
    pub type Establisher = DefaultEstablisher;

    /// The listener we are using
    #[derive(Debug)]
    pub struct DefaultListener(TcpListener);

    impl DefaultListener {
        /// Accepts a new incoming connection from this listener.
        pub async fn accept(
            &mut self,
            whitelist: &Option<HashSet<IpAddr>>,
            blacklist: &Option<HashSet<IpAddr>>,
        ) -> io::Result<(Duplex, SocketAddr)> {
            // accept
            let (sock, mut remote_addr) = self.0.accept().await?;
            let ip = normalize_ip(remote_addr.ip());
            if let Some(blacklist) = blacklist && blacklist.contains(&ip) {
                return Err(io::Error::new(io::ErrorKind::Other, "IP is blacklisted"));
            }
            if let Some(whitelist) = whitelist && !whitelist.contains(&ip) {
                return Err(io::Error::new(
                    io::ErrorKind::Other,
                    "A whitelist exists and the IP is not whitelisted",
                ));
            }
            // normalize address
            remote_addr.set_ip(remote_addr.ip().to_canonical());
            Ok((sock, remote_addr))
        }
    }

    /// Initiates a connection with given timeout in milliseconds
    #[derive(Debug)]
    pub struct DefaultConnector(MassaTime);

    impl DefaultConnector {
        /// Tries to connect to address
        ///
        /// # Argument
        /// * `addr`: `SocketAddr` we are trying to connect to.
        pub async fn connect(&mut self, addr: SocketAddr) -> io::Result<Duplex> {
            match timeout(self.0.to_duration(), TcpStream::connect(addr)).await {
                Ok(Ok(sock)) => Ok(sock),
                Ok(Err(e)) => Err(e),
                Err(e) => Err(io::Error::new(io::ErrorKind::TimedOut, e)),
            }
        }
    }

    /// Establishes a connection
    #[derive(Debug)]
    pub struct DefaultEstablisher;

    impl DefaultEstablisher {
        /// Creates an Establisher.
        pub fn new() -> Self {
            DefaultEstablisher {}
        }

        /// Gets the associated listener
        ///
        /// # Argument
        /// * `addr`: `SocketAddr` we want to bind to.
        pub async fn get_listener(&mut self, addr: SocketAddr) -> io::Result<DefaultListener> {
            Ok(DefaultListener(TcpListener::bind(addr).await?))
        }

        /// Get the connector with associated timeout
        ///
        /// # Argument
        /// * `timeout_duration`: timeout duration in milliseconds
        pub async fn get_connector(
            &mut self,
            timeout_duration: MassaTime,
        ) -> io::Result<DefaultConnector> {
            Ok(DefaultConnector(timeout_duration))
        }
    }

    impl Default for DefaultEstablisher {
        fn default() -> Self {
            Self::new()
        }
    }
}
