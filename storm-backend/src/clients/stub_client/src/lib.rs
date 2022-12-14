
use bramble-sync::{Message, MessageDestiny, Peer, ID, Client};


pub struct Stub {
}

impl Client for Stub{
    fn parse_dependencies(&self, msg: &Message) -> Vec<ID> {
        vec![]
    }
    //fn get_will_share_group_with_peers(Vec<Peer>, Identifier) -> Vec<bool>;
    fn get_will_share_group(&self, peer: &Peer, group: &ID) -> bool {
        true
    }
    fn validate(&self, msg: &Message) -> bool {
        true
    }
    fn get_message_destiny(&self, msg: &Message) -> MessageDestiny {
        MessageDestiny {}
    }
    fn get_next_message(&self) -> Message {
        Message {}
    }
    fn deliver(&self, msg: &Message) {
        ()
    }
}
