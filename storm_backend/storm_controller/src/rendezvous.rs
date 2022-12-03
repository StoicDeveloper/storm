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
use slice_as_array::{slice_as_array, slice_as_array_transmute};
use std::cell::OnceCell;
use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4};
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

pub type TorControl = AuthenticatedConn<
    TcpStream,
    Box<dyn Fn(AsyncEvent<'static>) -> Ready<std::result::Result<(), ConnError>>>,
>;

pub struct StormRendezvous {
    //control: AuthenticatedConn<TcpStream, AsyncEventCallback>,
    control: TorControl,
    peer_addr: OnceCell<OnionAddressV3>,
}

impl StormRendezvous {
    pub fn new(control: TorControl) -> Self
//fn new(control: &mut AuthenticatedConn<TcpStream, F>)
    //where
    //F: Fn(AsyncEvent<'static>) -> Future<Output = Result<(), torut::control::ConnError>>,
    {
        // start tor instance
        Self {
            control,
            peer_addr: OnceCell::new(),
        }
    }
}

const BRP_ID: &[u8] = b"org.briarproject.bramble.tor";
impl Id for StormRendezvous {
    const ID: &'static [u8] = BRP_ID;
}

fn calc_onion_addr(key: &[u8; 32]) -> torut::onion::OnionAddressV3 {
    // from slice, get bramble secret key
    // from secret key, get public key
    // from public key, get url
    let secret_key: bramble_crypto::SecretKey = (*key).into();
    let key_pair: bramble_crypto::KeyPair = secret_key.into();
    let public_key = key_pair.public();
    let tor_pub_key =
        &TorPublicKeyV3::from_bytes(slice_as_array!(public_key.as_ref(), [u8; 32]).unwrap())
            .unwrap();
    tor_pub_key.into()
}

fn tor_key_pair_from_public_key_slice(slice: &[u8; 32]) -> TorSecretKeyV3 {
    let secret_key: bramble_crypto::SecretKey = (*slice).into();
    let key_pair: bramble_crypto::KeyPair = secret_key.into();
    let mut buf = [0u8; 64];
    buf[0..32].copy_from_slice(key_pair.secret().as_ref());
    buf[32..64].copy_from_slice(key_pair.public().as_ref());
    buf.into()
}

impl Rendezvous for StormRendezvous {
    type Connection = TorSocket;

    //type ListenFuture = impl Future<Output = Result<Self::Connection>>;
    type ListenFuture = LocalBoxFuture<'static, Result<Self::Connection>>;
    type ConnectFuture = LocalBoxFuture<'static, Result<Self::Connection>>;

    fn prepare_endpoints(&mut self, stream_key: SymmetricKey, role: Role) {
        let mut key_material = [0u8; 64];
        bramble_crypto::stream(&stream_key, &mut key_material);
        let alice_seed = key_material[0..32].try_into().unwrap();
        let bob_seed = key_material[32..64].try_into().unwrap();
        let (target_addr, our_tor_key): (OnionAddressV3, &[u8; 32]) = match role {
            Role::Alice => (calc_onion_addr(&bob_seed), &alice_seed),
            Role::Bob => (calc_onion_addr(&alice_seed), &bob_seed),
        };

        block_on(self.control.add_onion_v3(
            &tor_key_pair_from_public_key_slice(our_tor_key),
            false,
            false,
            false,
            None,
            &mut Some((ONION_SERVICE_PORT, ONION_ADDR)).iter(),
        ))
        .unwrap();

        self.peer_addr.set(target_addr).unwrap();

        // calculate contact's url, and create own hidden service using seed
    }

    fn listen(&mut self) -> Self::ListenFuture {
        Box::pin(async_listen())
    }

    fn connect(&mut self) -> Self::ConnectFuture {
        Box::pin(async_connect(self.peer_addr.get().unwrap().clone()))
    }
}

async fn async_listen() -> Result<TorSocket> {
    let server_listener = TcpListener::bind("127.0.0.1:1917").await.unwrap();
    Ok(server_listener.accept().await.unwrap().0.into())
}

async fn async_connect(addr: OnionAddressV3) -> Result<TorSocket> {
    Ok(Socks5Stream::connect(
        ("127.0.0.1", TOR_PROXY_PORT),
        addr.get_address_without_dot_onion() + ".onion",
    )
    .await
    .unwrap()
    .into_inner()
    .into())
}
