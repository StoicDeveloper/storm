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
    pub key: KeyPair,
    pub groups: BisetMap<PublicKey, String>,
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
        let contacts = Self::load_contact_groups(&conn, &name);
        Self {
            conn,
            key,
            contacts,
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
            key BLOB,
            name TEXT,
            PRIMARY KEY(name)
        );",
            (),
        )
        .unwrap();

        conn.execute(
            "
        CREATE TABLE sharing_groups (
            name TEXT,
            group_name TEXT,
            PRIMARY KEY(name, group_name),
            FOREIGN KEY(name) REFERENCES peers(name) ON DELETE CASCADE
        );",
            (),
        )
        .unwrap();
    }

    fn add_peer(&mut self, name: &str, key: PublicKey) -> KeyPair {
        let mut conn = &mut self.conn;
        conn.execute(
            "
            INSERT INTO peers
            VALUES (?, ?);",
            params![name, key],
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
            SELECT * FROM sharing_groups 
            WHERE group_name = ?
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

    pub fn groups(&self) -> Vec<String> {
        self.contacts
            .flat_collect()
            .into_iter()
            .map(|(_, group)| group)
            .collect::<HashSet<String>>()
            .into_iter()
            .collect()
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
