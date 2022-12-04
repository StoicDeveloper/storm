use futures::io::{AsyncReadExt, AsyncWriteExt};
use futures::select;
use futures::{AsyncRead as StdAsyncRead, AsyncWrite as StdAsyncWrite};
use gag::BufferRedirect;
use libtor::*;
use portpicker::pick_unused_port;
use std::collections::VecDeque;
use std::future::Future;
use std::io::{Read, Write};
use std::ops::DerefMut;
use std::pin::Pin;
use std::sync::*;
use std::task::{Poll, Waker};
use std::thread;
use std::thread::JoinHandle;
use tokio::io::AsyncWrite as TokioAsyncWrite;
use tokio::net::TcpStream;

// Readings:
// https://osamaelnaggar.com/blog/proxying_application_traffic_through_tor/
// https://manpages.debian.org/stretch/tor/torrc.5.en.html
// https://www.linuxjournal.com/content/tor-hidden-services
// https://docs.rs/async-socks5/latest/async_socks5/
//

pub const CONTROL_PORT: u16 = 9151;
pub const SERVICE_PORT: u16 = 9150;
type TorHandle = JoinHandle<Result<u8, libtor::Error>>;

struct SharedTorOutputState {
    waker: Option<Waker>,
    output: VecDeque<TorOutput>,
}

// Try tracking tor state by:
// implement several futures that wait for the desired state to be reached
// Create a struct that contains the output field and a waker
// When the output field is concatenated to, wake the waker
// A future created will contain a clone of the output state,
//
// Polling just calls update state and then checks the new state
// Share SharedTorOutputState is contained in the output thread and in the tor instance struct
// The future only needs to get the waker from the SharedTorOutputState, and call it if pending

#[derive(Debug)]
enum TorOutput {
    NoChange,
    Started,
    Failed,
    Error,
    Exited,
}

impl TorOutput {
    fn process_output(line: &str) -> TorOutput {
        use TorOutput::*;
        if line.contains("Bootstrapped 100% (done): Done") {
            Started
        } else {
            NoChange
        }
    }
}

pub struct TorInstance {
    tor_monitor: JoinHandle<()>,
    output: Arc<Mutex<SharedTorOutputState>>,
    pub control_socket: Option<std::net::TcpStream>,
    pub control_port: u16,
    service_port: u16,
    started: bool,
    finished: bool,
    error: bool,
}
impl TorInstance {
    pub fn get_control_port(&self) -> u16 {
        self.control_port
    }
    pub async fn get_control_socket(&self) -> TcpStream {
        eprintln!("{}", self.control_port);
        // For some reason making the tokio stream directly would wait forever
        let tcp = std::net::TcpStream::connect(("127.0.0.1", self.control_port)).unwrap();
        tcp.set_nonblocking(true).unwrap();
        eprintln!("made socket");
        TcpStream::from_std(tcp).unwrap()
        //TcpStream::connect(("127.0.0.1", self.control_port))
        //.await
        //.unwrap()
    }
    //pub fn set_control_socket(&mut self) {
    //self.control_socket =
    //Some(std::net::TcpStream::connect(("127.0.0.1", self.control_port)).unwrap());
    //}
    pub fn new(services: Vec<u16>) -> Self {
        let output = Arc::new(Mutex::new(SharedTorOutputState {
            waker: None,
            output: VecDeque::new(),
        }));
        let control_port = pick_unused_port().unwrap();
        let output_queue = output.clone();
        let tor_monitor = thread::spawn(move || {
            let mut buf = BufferRedirect::stdout().unwrap();
            let mut tor = Tor::new();
            tor.flag(TorFlag::DataDirectory("/tmp/tor-rust".into()));
            tor.flag(TorFlag::SocksPort(19050));
            //tor.flag(TorFlag::ControlPort(9151));
            tor.flag(TorFlag::ControlPort(control_port));
            tor.flag(TorFlag::ControlPortWriteToFile("control".to_string()));
            //tor.flag(TorFlag::CookieAuthentication(TorBool::True));
            let mut count = 0;
            for port in services {
                tor.flag(TorFlag::HiddenServiceDir(
                    format!("hidden_services/{}", count).into(),
                ));
                tor.flag(TorFlag::HiddenServiceVersion(HiddenServiceVersion::V3));
                tor.flag(TorFlag::HiddenServicePort(
                    TorAddress::Port(port),
                    None.into(),
                ));
                count += 1;
            }
            tor.start_background();
            let mut output = String::new();
            //eprintln!("hello world");
            loop {
                let res = buf.read_to_string(&mut output);
                match res {
                    Ok(0) => {
                        std::thread::sleep(std::time::Duration::from_millis(300));
                        continue;
                    }
                    Err(_) => {
                        eprintln!("read error");
                        break;
                    }
                    Ok(n) => {
                        //eprintln!("read {} bytes", n);
                    }
                }
                let mut out = output_queue.lock().unwrap();
                for line in output.lines() {
                    eprintln!("{}", line);
                    (*out).output.push_back(TorOutput::process_output(line));
                }
                if let Some(waker) = (*out).waker.take() {
                    //eprintln!("waking in loop");
                    waker.wake();
                }
                output.clear();
            }
            eprintln!("loop ended");
            let mut out = output_queue.lock().unwrap();
            if let Some(waker) = (*out).waker.take() {
                eprintln!("waking out of loop");
                waker.wake();
            }
            (*out).output.push_back(TorOutput::Exited);
        });

        Self {
            tor_monitor,
            output,
            control_socket: None,
            control_port,
            service_port: SERVICE_PORT,
            started: false,
            finished: false,
            error: false,
        }
    }

    pub fn started(&mut self) -> TorStarting {
        TorStarting { instance: self }
    }

    fn process_output(&mut self) {
        while let Some(change) = self.output.lock().unwrap().output.pop_front() {
            let c = match &change {
                TorOutput::NoChange => {}
                TorOutput::Started => self.started = true,
                TorOutput::Failed => {
                    self.error = true;
                    self.finished = true;
                }
                TorOutput::Error => self.error = true,
                TorOutput::Exited => self.finished = true,
            };

            if !is_variant!(change, TorOutput::NoChange) {
                eprintln!("{:?}", change);
            }
        }
    }
}

pub struct TorStarting<'t> {
    instance: &'t mut TorInstance,
}

impl<'t> Future for TorStarting<'t> {
    type Output = ();

    fn poll(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Self::Output> {
        self.instance.process_output();
        let mut state = self.instance.output.lock().unwrap();
        //println!("poll");
        //eprintln!("poll");
        if self.instance.started {
            //eprintln!("ready");
            Poll::Ready(())
        } else {
            // May need an array of wakers
            //eprintln!("pending");
            state.waker = Some(cx.waker().clone());
            Poll::Pending
        }
    }
}

pub struct TorSocket {
    inner: tokio::net::TcpStream,
}

impl From<TcpStream> for TorSocket {
    fn from(sock: TcpStream) -> Self {
        Self { inner: sock }
    }
}

impl StdAsyncWrite for TorSocket {
    fn poll_write(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &[u8],
    ) -> std::task::Poll<std::io::Result<usize>> {
        Pin::new(&mut self.inner).poll_write(cx, buf)
    }

    fn poll_flush(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        Pin::new(&mut self.inner).poll_flush(cx)
    }

    fn poll_close(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        Pin::new(&mut self.inner).poll_shutdown(cx)
    }
}

impl StdAsyncRead for TorSocket {
    fn poll_read(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &mut [u8],
    ) -> std::task::Poll<std::io::Result<usize>> {
        todo!()
    }
}

pub struct TorControl {
    inner: std::net::TcpStream,
}

impl TorControl {
    pub fn new(port: u16) -> Self {
        let mut control = Self {
            inner: std::net::TcpStream::connect(("127.0.0.1", port)).unwrap(),
        };
        control.auth();
        control
    }
    fn send(&mut self, command: &[u8]) {
        eprintln!("{}", std::str::from_utf8(command).unwrap().to_string());
        self.inner.write_all(command).unwrap();
    }
    fn reply(&mut self) -> String {
        let mut buf = [0u8; 200];
        let n = self.inner.read(&mut buf).unwrap();
        let reply = std::str::from_utf8(&buf[0..n]).unwrap().to_string();
        eprintln!("{}", &reply);
        reply
    }
    fn auth(&mut self) -> bool {
        self.send(b"AUTHENTICATE \r\n");
        self.reply().contains("250 OK")
    }
    pub fn add_onion_v3(
        &mut self,
        key: torut::onion::TorSecretKeyV3,
        port: u16,
    ) -> bramble_common::Result<()> {
        let mut res = String::new();
        res.push_str("ADD_ONION ED25519-V3:");
        res.push_str(&base64::encode(&key.as_bytes()));
        res.push_str(" Flags=DiscardPK ");
        res.push_str(&format!("Port={},{}:{}", 1917, "127.0.0.1", port));
        res.push_str(" \r\n");

        self.send(res.as_bytes());
        assert!(self.reply().contains("250 OK"));
        Ok(())
    }
}

#[cfg(test)]
mod test {
    use crate::tor::*;
    use futures::executor::block_on;
    use futures::FutureExt;
    use futures_timer::Delay;
    use gag::BufferRedirect;
    use libtor::*;
    use std::io::Read;
    use std::net::SocketAddr;
    use std::time::Duration;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;

    use std::sync::mpsc;
    use std::thread::{self, JoinHandle};

    fn create_tor_foreground() {
        match Tor::new()
            .flag(TorFlag::DataDirectory("/tmp/tor-rust".into()))
            .flag(TorFlag::SocksPort(19050))
            .flag(TorFlag::HiddenServiceDir("hidden_services/1".into()))
            .flag(TorFlag::HiddenServicePort(
                TorAddress::Port(8004),
                Some(TorAddress::AddressPort("127.0.0.1".into(), 5000)).into(),
            ))
            .flag(TorFlag::HiddenServiceVersion(HiddenServiceVersion::V3))
            .flag(TorFlag::HiddenServiceDir("hidden_services/2".into()))
            .flag(TorFlag::HiddenServicePort(
                TorAddress::Port(8004),
                None.into(),
            ))
            .flag(TorFlag::HiddenServiceVersion(HiddenServiceVersion::V3))
            .flag(TorFlag::HiddenServiceDir("hidden_services/3".into()))
            .flag(TorFlag::HiddenServicePort(
                TorAddress::Port(8004),
                None.into(),
            ))
            .flag(TorFlag::HiddenServiceVersion(HiddenServiceVersion::V3))
            .start()
        {
            Ok(v) => println!("started tor"),
            Err(e) => println!("failed to start tor"),
        }
    }
    use async_socks5::{connect, AddrKind, SocksListener};
    use tokio::{io::BufStream, net::TcpStream};

    // This creates a dependency on tokio. To remove it in the future, if desired, look at this
    // link: https://thomask.sdf.org/blog/2021/03/08/bridging-sync-async-code-in-rust.html
    #[tokio::test]
    #[ignore]
    async fn create_tor_socket() {
        // uses async_socks5
        create_connection();
        // Create Tcp conn to socks port
        let proxy_addr = "127.0.0.1:19050".parse::<SocketAddr>().unwrap();
        let server_url = std::fs::read_to_string("hidden_services/0/hostname").unwrap();
        let server_addr = AddrKind::Domain(server_url, 8004);
        //let local_server_add = AddrKind::Domain("127.0.0.1".to_string(), 8004);
        let local_server_add = ("127.0.0.1", 8004);

        let client = TcpStream::connect(&proxy_addr).await.unwrap();
        let client = BufStream::new(client);
        let client = SocksListener::bind(client, server_addr, None)
            .await
            .unwrap();
        let mut server = TcpStream::connect(&local_server_add).await.unwrap();
        let (mut client, _) = client.accept().await.unwrap();
        server.write_all(b"blag").await.unwrap();
        let mut buf = [0; 4];
        client.read_exact(&mut buf).await.unwrap();
        assert_eq!(&buf, b"blag");
        //eprintln!("created stream");
        //let mut stream = BufStream::new(stream);
        //let listener = SocksListener::bind(
        //let req = connect(&mut stream, (&onion_url, 8004), None);
    }

    #[test]
    #[ignore]
    fn create_connection() {
        //use libtor::{HiddenServiceVersion, Tor, TorAddress, TorFlag};
        let mut tor = TorInstance::new(vec![8004]);
        block_on(async {
            select!(
            _ = tor.started().fuse() => {},
            _ = Delay::new(Duration::from_secs(30)).fuse() => panic!(),
            )
        });

        //block_on(create_tor_socket());

        // connect to hidden service
        // listen for socks connection on hidden service port
        // send socks connect request over socksport
        // see what happens
        // print off if conn successful
    }

    use std::net::{Ipv4Addr, SocketAddrV4};
    //#[tokio::test]
    //fn conn_rev_proxy() {
    //create_connection();
    //let socks_addr = SocketAddr::V4(SocketAddr::new(Ipv4Addr::LOCALHOST, 19050));
    //let local_addr =
    //let listener = TcpListener::bind(local_addr).await.unwrap();

    //}

    use tokio_socks::tcp::{Socks5Listener, Socks5Stream};
    #[tokio::test]
    async fn conn_tokio_socks() {
        //let proxy_addr = SocketAddr::V4(SocketAddr::new(Ipv4Addr::LOCALHOST, 19050));
        create_connection();
        let proxy_addr = ("127.0.0.1", 19050);
        let target_addr = (
            std::fs::read_to_string("hidden_services/0/hostname")
                .unwrap()
                .strip_suffix("\n")
                .unwrap()
                .to_string(),
            8004,
        );
        eprintln!("{:?}", target_addr);
        //block_on(Delay::new(Duration::from_secs(120)));
        //Socks5Listener::bind(proxy_addr, target_addr).await.unwrap();
        let server_listener = TcpListener::bind("127.0.0.1:8004").await.unwrap();
        eprintln!("listener");
        let client_request = Socks5Stream::connect(proxy_addr, target_addr);
        let (mut client_sock, (mut server_sock, server_addr)) = tokio::select!(
            (client_stream, server_sock) = async {
                tokio::join!(client_request, server_listener.accept())
            } =>
            {(client_stream.unwrap().into_inner(), server_sock.unwrap())},
            _ = Delay::new(Duration::from_secs(300)).fuse() => {
                eprintln!("timeout");
                panic!();
            }
        );
        client_sock.write_all(b"sending client msg").await.unwrap();
        let mut buf = [0u8; 50];
        let n = server_sock.read(&mut buf).await.unwrap();
        eprintln!(
            "server received {}",
            std::str::from_utf8(&buf[0..n]).unwrap()
        );
        server_sock.write_all(b"sending server msg").await.unwrap();
        let mut buf = [0u8; 50];
        let n = client_sock.read(&mut buf).await.unwrap();
        eprintln!(
            "client received {}",
            std::str::from_utf8(&buf[0..n]).unwrap()
        );
    }

    // create hidden services

    // pass messages from one hidden service to another

    // create rendezvous connection over hidden service

    // pass data from one rendezvous socket to another
}
