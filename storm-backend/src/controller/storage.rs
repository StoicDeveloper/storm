use fallible_iterator::FallibleIterator;
use std::{
    collections::{HashMap, HashSet},
    path::Path,
};

use bisetmap::BisetMap;
use bramble_crypto::{KeyPair, PublicKey, SecretKey, KEY_LEN};
use rand::thread_rng;
use rusqlite::{params, Connection};

pub struct Profile {
    pub conn: Connection,
    pub name: String,
    pub key: KeyPair,
    pub groups: HashSet<String>,
    pub peer_groups: BisetMap<PublicKey, String>,
    pub peers: HashMap<String, PublicKey>,
}

impl Profile {
    pub fn load(name: String) -> Profile {
        // create db conn
        // check if file exists, if not, initialize
        let path = "./profiles.db";
        let exists = Path::new(path).exists();
        let conn = Connection::open(path).unwrap();
        if !exists {
            Self::init(&conn);
        }
        let key = Self::load_key(&conn, &name);
        let groups = Self::load_groups(&conn, &name);
        let peer_groups = Self::load_contact_groups(&conn, &name);
        let peers = Self::load_peers(&conn, &name);
        Self {
            conn,
            name,
            key,
            groups,
            peer_groups,
            peers,
        }
    }

    fn init(conn: &Connection) {
        conn.execute(
            "
        CREATE TABLE profiles (
            name TEXT PRIMARY KEY,
            secret BLOB
        );",
            (),
        )
        .unwrap();

        //conn.execute(
        //"
        //CREATE TABLE contacts (
        //key BLOB,
        //name TEXT,
        //PRIMARY KEY(key, name),
        //);",
        //(),
        //).unwrap();

        conn.execute(
            "
        CREATE TABLE peers (
            user TEXT,
            name TEXT UNIQUE,
            key BLOB,
            PRIMARY KEY(user, key),
            FOREIGN KEY(user) REFERENCES profiles(name) ON DELETE CASCADE
        );",
            (),
        )
        .unwrap();

        conn.execute(
            "
        CREATE TABLE msg_groups (
            user TEXT,
            name TEXT,
            PRIMARY KEY(user, name),
            FOREIGN KEY(user) REFERENCES profiles(name) ON DELETE CASCADE
        );",
            (),
        )
        .unwrap();

        conn.execute(
            "
        CREATE TABLE sharing_groups (
            user TEXT,
            peer BLOB,
            group_name TEXT,
            PRIMARY KEY(user, peer, group_name),
            FOREIGN KEY(user, peer) REFERENCES peers(user, key) ON DELETE CASCADE
        );",
            (),
        )
        .unwrap();
    }

    pub fn add_peer(&mut self, name: &str, key: PublicKey) {
        let mut conn = &mut self.conn;
        conn.execute(
            "
            INSERT INTO peers
            VALUES (?, ?, ?);",
            params![self.name, name, key.as_ref()],
        );
    }

    pub fn add_group(&mut self, group: &str) {
        let mut conn = &mut self.conn;
        conn.execute(
            "
            INSERT INTO msg_groups
            VALUES (?, ?);",
            params![self.name, group],
        );
    }
    pub fn add_peer_to_group(&mut self, peer: PublicKey, group: &str) {
        let mut conn = &mut self.conn;
        conn.execute(
            "
            INSERT INTO sharing_groups
            VALUES (?, ?, ?);",
            params![self.name, peer.as_ref(), group],
        );
    }
    fn load_key(conn: &Connection, name: &str) -> KeyPair {
        let res: Result<[u8; KEY_LEN], rusqlite::Error> = conn.query_row(
            "
            SELECT secret
            FROM profiles
            WHERE name = ?;",
            [name],
            //|row| Ok((key_from_sql(row.get_unwrap(0))))
            |row| row.get(0),
        );
        match res {
            Ok(key) => {
                let secret: SecretKey = key.into();
                secret.into()
            }
            Err(_) => {
                let rng = &mut thread_rng();
                let key = KeyPair::generate(rng);
                insert_key(&conn, name, &key);
                key
            }
        }
    }

    fn load_contact_groups(conn: &Connection, name: &str) -> BisetMap<PublicKey, String> {
        let mut stmt = conn
            .prepare(
                "
            SELECT peer, group_name FROM sharing_groups 
            WHERE user = ?
            ;",
            )
            .unwrap();
        let contact_groups: Vec<(PublicKey, String)> = stmt
            .query([name])
            .unwrap()
            .map(|row| Ok((row.get_unwrap(0), row.get_unwrap(1))))
            .collect()
            .unwrap();
        let mut map: BisetMap<PublicKey, String> = BisetMap::new();
        contact_groups
            .into_iter()
            .for_each(|(key, name)| map.insert(key, name));
        //.for_each(|(key, name)| match map.get_mut(&key) {
        //Some(names) => names.push(name),
        //None => {
        //map.insert(key, vec![name]);
        //}
        //});
        map
    }

    //pub fn group_list(&self) -> Vec<String> {
    //self.groups
    //.flat_collect()
    //.into_iter()
    //.map(|(_, group)| group)
    //.collect::<HashSet<String>>()
    //.into_iter()
    //.collect()
    //}

    fn load_peers(conn: &Connection, name: &str) -> HashMap<String, PublicKey> {
        let mut stmt = conn
            .prepare(
                "
            SELECT name, key
            FROM profiles
            WHERE user = ?
            ;",
            )
            .unwrap();
        let peers: Vec<(String, PublicKey)> = stmt
            .query([name])
            .unwrap()
            .map(|row| Ok((row.get_unwrap(0), row.get_unwrap(1))))
            .collect()
            .unwrap();
        peers.into_iter().collect()
    }
    pub fn load_groups(conn: &Connection, name: &str) -> HashSet<String> {
        let mut stmt = conn
            .prepare(
                "
            SELECT name
            FROM msg_groups
            WHERE user = ?
            ;",
            )
            .unwrap();
        stmt.query([name])
            .unwrap()
            .map(|row| Ok(row.get_unwrap(0)))
            .collect()
            .unwrap()
    }
}

fn insert_key(conn: &Connection, name: &str, key: &KeyPair) {
    conn.execute(
        "
        INSERT INTO profiles
        VALUES (?, ?);",
        params![name, key.secret().as_ref()],
    );
}
