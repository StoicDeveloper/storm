use crate::controller::tor::*;
use bramble_common::transport::Id;
use bramble_common::Result;
use bramble_crypto::PublicKey;
use bramble_crypto::Role;
use bramble_crypto::SymmetricKey;
use bramble_rendezvous::Rendezvous;
use futures::executor::block_on;
use futures::future::ready;
use futures::future::LocalBoxFuture;
use futures::future::Ready;
use futures::Future;
use futures::FutureExt;
use futures_timer::Delay;
use log::{debug, error, info, trace, warn};
use portpicker::pick_unused_port;
use slice_as_array::{slice_as_array, slice_as_array_transmute};
use std::cell::OnceCell;
use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4};
use std::rc::Rc;
use std::sync::Arc;
use std::sync::Mutex;
use std::time::Duration;
use tokio::net::TcpListener;
use tokio::net::TcpStream;
use tokio_socks::tcp::Socks5Stream;
use torut::control::*;
use torut::onion::*;

use crate::controller::tor::TorSocket;

// Implement Rendezvous trait on TorConnection

const ID: &str = "org.briarproject.bramble.tor";
const ONION_SERVICE_PORT: u16 = 1917;
const TOR_PROXY_PORT: u16 = 19050;
const ONION_ADDR: SocketAddr = SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::LOCALHOST, 1917));

fn do_nothing(event: AsyncEvent<'static>) -> Ready<Result<()>> {
    ready(Ok(()))
}

//pub type TorControl = AuthenticatedConn<
//TcpStream,
//Box<dyn Fn(AsyncEvent<'static>) -> Ready<std::result::Result<(), ConnError>>>,
//>;

pub struct StormRendezvous {
    //control: AuthenticatedConn<TcpStream, AsyncEventCallback>,
    name: String,
    control: Arc<Mutex<TorControl>>,
    peer: PublicKey,
    peer_addr: OnceCell<OnionAddressV3>,
    listener: Option<LocalBoxFuture<'static, Result<TcpListener>>>,
}

impl StormRendezvous {
    pub fn new(name: String, control: Arc<Mutex<TorControl>>, peer: PublicKey) -> Self
//fn new(control: &mut AuthenticatedConn<TcpStream, F>)
    //where
    //F: Fn(AsyncEvent<'static>) -> Future<Output = Result<(), torut::control::ConnError>>,
    {
        // start tor instance
        Self {
            name,
            control,
            peer,
            peer_addr: OnceCell::new(),
            listener: None,
        }
    }
}

const BRP_ID: &[u8] = b"org.briarproject.bramble.tor";
impl Id for StormRendezvous {
    const ID: &'static [u8] = BRP_ID;
}

fn calc_onion_addr(seed: &[u8; 32]) -> torut::onion::OnionAddressV3 {
    // from slice, get bramble secret key
    // from secret key, get public key
    // from public key, get url
    //let secret_key: bramble_crypto::SecretKey = (*seed).into();
    //let key_pair: bramble_crypto::KeyPair = secret_key.into();
    //let public_key = key_pair.public();
    //let tor_pub_key =
    //&TorPublicKeyV3::from_bytes(slice_as_array!(public_key.as_ref(), [u8; 32]).unwrap())
    //.unwrap();
    //tor_pub_key.into()

    // Instead
    // use seed as ed25519 private key, convert to ed25519 public key
    // convert public key to TorPublicKeyV3, convert that to onion
    let secret: &ed25519_dalek::SecretKey = &ed25519_dalek::SecretKey::from_bytes(seed).unwrap();
    let public: ed25519_dalek::PublicKey = secret.into();
    let tor_public = &TorPublicKeyV3::from_bytes(&public.to_bytes()).unwrap();
    tor_public.into()
}

fn tor_key_pair_from_public_key_slice(slice: &[u8; 32]) -> TorSecretKeyV3 {
    let secret_key: bramble_crypto::SecretKey = (*slice).into();
    let key_pair: bramble_crypto::KeyPair = secret_key.into();
    let mut buf = [0u8; 64];
    buf[0..32].copy_from_slice(key_pair.secret().as_ref());
    buf[32..64].copy_from_slice(key_pair.public().as_ref());
    buf.into()
}

fn get_tor_secret(seed: &[u8; 32]) -> TorSecretKeyV3 {
    // Get normal ed25519 secret key,
    // convert to expanded secret key

    let secret: ed25519_dalek::SecretKey = ed25519_dalek::SecretKey::from_bytes(seed).unwrap();
    let expanded = ed25519_dalek::ExpandedSecretKey::from(&secret);
    expanded.to_bytes().into()
}

impl Rendezvous for StormRendezvous {
    type Connection = TorSocket;

    //type ListenFuture = impl Future<Output = Result<Self::Connection>>;
    type ListenFuture = LocalBoxFuture<'static, Result<Self::Connection>>;
    type ConnectFuture = LocalBoxFuture<'static, Result<Self::Connection>>;

    #[allow(unused_must_use)]
    fn prepare_endpoints(&mut self, stream_key: SymmetricKey, role: Role) {
        trace!(target: "rendezvous", "{} preparing endpoints", self.name);
        debug!(target: "crypto", "{}, {:?}", self.name,stream_key);
        let mut key_material = [0u8; 64];
        bramble_crypto::stream(&stream_key, &mut key_material);
        let alice_seed = key_material[0..32].try_into().unwrap();
        let bob_seed = key_material[32..64].try_into().unwrap();
        //let (target_addr, our_tor_key): (OnionAddressV3, &[u8; 32]) = match role {
        //Role::Alice => (calc_onion_addr(&bob_seed), &alice_seed),
        //Role::Bob => (calc_onion_addr(&alice_seed), &bob_seed),
        //};
        let (peer_secret, our_secret): (TorSecretKeyV3, TorSecretKeyV3) = match role {
            Role::Alice => (get_tor_secret(bob_seed), get_tor_secret(alice_seed)),
            Role::Bob => (get_tor_secret(alice_seed), get_tor_secret(bob_seed)),
        };
        //eprintln!("their secret {:?}", &peer_secret.as_bytes());
        //eprintln!("our secret {:?}", &our_secret.as_bytes());

        let mut tor_cont = self.control.lock().unwrap();
        let port = pick_unused_port().unwrap();
        self.listener.insert(Box::pin(
            TcpListener::bind(SocketAddrV4::new(Ipv4Addr::LOCALHOST, port))
                .map(|res| res.map_err(|io_err| bramble_common::Error::Io(io_err))),
        ));
        //.add_onion_v3(
        //&tor_key_pair_from_public_key_slice(our_tor_key),
        //false,
        //false,
        //false,
        //None,
        //&mut Some((ONION_SERVICE_PORT, ONION_ADDR)).iter(),
        //
        //)
        let our_onion = our_secret.public().get_onion_address();
        tor_cont
            //.add_onion_v3(tor_key_pair_from_public_key_slice(our_tor_key), port)
            .add_onion_v3(our_secret, port)
            .unwrap();
        let their_onion = peer_secret.public().get_onion_address();
        self.peer_addr.set(their_onion).unwrap();
        trace!(target: "network", "{} opening port {} at url {}", self.name, port, our_onion);
        trace!(target: "network", "{} seeking peer at url {}", self.name, their_onion);

        // calculate contact's url, and create own hidden service using seed
    }

    fn listen(&mut self) -> Self::ListenFuture {
        Box::pin(async_listen(
            self.name.clone(),
            self.listener.take().unwrap(),
            self.peer,
        ))
    }

    fn connect(&mut self) -> Self::ConnectFuture {
        Box::pin(async_connect(
            self.name.clone(),
            self.peer_addr.get().unwrap().clone(),
            self.peer,
        ))
    }
}

impl bramble_common::transport::Latency for StormRendezvous {
    const MAX_LATENCY_SECONDS: u32 = 1;
}

use futures::select;
async fn async_listen(
    name: String,
    listener: LocalBoxFuture<'static, Result<TcpListener>>,
    peer: PublicKey,
) -> Result<TorSocket> {
    //let listener = listener.await?;
    //let sock;
    //let addr;
    //loop {
    //select!(

    //res = listener.accept().fuse()
    //=> {let (a, b) = res.unwrap(); sock = a; addr = b;break;},
    //_ = Delay::new(Duration::from_secs(1)).fuse() => println!("{} listening", &name)
    //);
    //}
    let (sock, addr) = listener.await?.accept().await?;
    trace!(target: "network", "{} accepted connection at {}", name, addr);
    Ok(TorSocket::from_tcp(sock, peer))
}

async fn async_connect(name: String, addr: OnionAddressV3, peer: PublicKey) -> Result<TorSocket> {
    let socks_stream =
        //log_future(
        Socks5Stream::connect(
            ("127.0.0.1", TOR_PROXY_PORT),
            addr.get_address_without_dot_onion() + ".onion:1917",
        )
        //1,
        //format!("{} connecting", &name).as_str(),
    //)
    .await?;

    //TODO: Handle failure, ex. host unreachable

    trace!(target: "network", "{} connected to proxy {:?}", name, &socks_stream);
    Ok(TorSocket::from_tcp(socks_stream.into_inner(), peer))
}

async fn log_future<F: Future>(fut: F, frequency: u64, msg: &str) -> F::Output {
    let mut fut = Box::pin(fut.fuse());
    loop {
        select!(
        res = fut
        => return res,
        _ = Delay::new(Duration::from_secs(frequency)).fuse() => println!("{}", msg)
                );
    }
}
