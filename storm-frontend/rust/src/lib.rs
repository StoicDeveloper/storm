mod bridge; /* AUTO INJECTED BY flutter_rust_bridge. This line may not be accurate, and you can change it according to your needs. */
//use flutter_rust_bridge::ZeroCopyBuffer;

use storm_controller::start;
use storm_controller::createAccount;
use storm_controller::createAlias;
use storm_controller::login;
use storm_controller::getGroups;
use storm_controller::getGroupMessages;
use storm_controller::sendGroupMessage;
use storm_controller::setGroupSettings;

pub fn start() -> () {
    start()
}

pub fn createAccount(num1: u8, num2: u8) -> () {
    createAccount()
}

pub fn createAlias() -> () {
    createAlias()
}

pub fn login(username: String, password: String) -> () {
    login()
}

pub fn getGroups(alias: u16) -> () {
    getGroups()
}

pub fn getGroupMessages(group: u16) -> () {
    getGroupMessages()
}

pub fn sendGroupMessage(group: u16, msg: String) -> () {
    sendGroupMessage()
}

pub fn setGroupSettings(group: u16, msg: String) -> () {
    setGroupSettings()
}

// need some way of including the client and plugin apis
