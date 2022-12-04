use crate::tor::*;
use bramble_common::transport::Id;
use bramble_common::Result;
use bramble_crypto::Role;
use bramble_crypto::SymmetricKey;
use bramble_rendezvous::Rendezvous;
use futures::executor::block_on;
use futures::future::ready;
use futures::future::LocalBoxFuture;
use futures::future::Ready;
use futures::Future;
use portpicker::pick_unused_port;
use slice_as_array::{slice_as_array, slice_as_array_transmute};
use std::cell::OnceCell;
use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4};
use std::rc::Rc;
use std::sync::Mutex;
use tokio::net::TcpListener;
use tokio::net::TcpStream;
use tokio_socks::tcp::Socks5Stream;
use torut::control::*;
use torut::onion::*;

use crate::tor::TorSocket;

// Implement Rendezvous trait on TorConnection

const ID: &str = "org.briarproject.bramble.tor";
const ONION_SERVICE_PORT: u16 = 1917;
const TOR_PROXY_PORT: u16 = 9050;
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
    control: Rc<Mutex<TorControl>>,
    peer_addr: OnceCell<OnionAddressV3>,
    listener: Option<TcpListener>,
}

impl StormRendezvous {
    pub fn new(control: Rc<Mutex<TorControl>>) -> Self
//fn new(control: &mut AuthenticatedConn<TcpStream, F>)
    //where
    //F: Fn(AsyncEvent<'static>) -> Future<Output = Result<(), torut::control::ConnError>>,
    {
        // start tor instance
        Self {
            control,
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
        eprintln!("prepare_endpoints");

        eprintln!("{:?}", stream_key);
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
        //eprintln!("their address {}", &target_addr);
        eprintln!("their secret {:?}", &peer_secret.as_bytes());
        eprintln!("our secret {:?}", &our_secret.as_bytes());

        let mut tor_cont = self.control.lock().unwrap();
        let port = pick_unused_port().unwrap();
        self.listener
            .insert(block_on(TcpListener::bind(&format!("127.0.0.1:{}", port))).unwrap());
        //.add_onion_v3(
        //&tor_key_pair_from_public_key_slice(our_tor_key),
        //false,
        //false,
        //false,
        //None,
        //&mut Some((ONION_SERVICE_PORT, ONION_ADDR)).iter(),
        //
        //)
        tor_cont
            //.add_onion_v3(tor_key_pair_from_public_key_slice(our_tor_key), port)
            .add_onion_v3(our_secret, port)
            .unwrap();

        let onion = peer_secret.public().get_onion_address();
        eprintln!("Their address: {}", &onion);
        self.peer_addr.set(onion).unwrap();
        eprintln!("end prepare_endpoints");

        // calculate contact's url, and create own hidden service using seed
    }

    fn listen(&mut self) -> Self::ListenFuture {
        eprintln!("listen");
        let res = Box::pin(async_listen(self.listener.take().unwrap()));
        eprintln!("end listen");
        res
    }

    fn connect(&mut self) -> Self::ConnectFuture {
        eprintln!("connect");
        let res = Box::pin(async_connect(self.peer_addr.get().unwrap().clone()));
        eprintln!("end connect");
        res
    }
}

impl bramble_common::transport::Latency for StormRendezvous {
    const MAX_LATENCY_SECONDS: u32 = 60;
}

async fn async_listen(listener: TcpListener) -> Result<TorSocket> {
    Ok(listener.accept().await.unwrap().0.into())
}

async fn async_connect(addr: OnionAddressV3) -> Result<TorSocket> {
    Ok(Socks5Stream::connect(
        ("127.0.0.1", TOR_PROXY_PORT),
        addr.get_address_without_dot_onion() + ".onion:1917",
    )
    .await?
    .into_inner()
    .into())
}
