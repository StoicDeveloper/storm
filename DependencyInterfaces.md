# Dependency Interfaces

A specification for the interfaces that each Storm dependency relationship consists of.

## Dependencies

1. Platform -> BQP
2. Platform -> BRP
3. Platform -> BTP
4. Platform -> BSP
5. SyncClient -> Platform
6. Groups -> SyncClient
7. ClientPlugins -> SyncClient
8. App -> SyncClient
9. App -> ClientPlugins

## Interfaces

1. Platform -> BQP

whatever is needed to get the public key from a picture

2. Platform -> BRP

let stormBRP = StormBRP::start()
let rendezvousSocket = StormBRP::rendezvous()
StormBRP::stop()

rendezvousSocket.read()
rendezvousSocket.write()
rendezvousSocket.close()

3. Platform -> BTP

let transportSocket = StormBTP::makeSocket(socket)
transportSocket.read()
transportSocket.write()
transportSocket.close()

4. Platform -> BSP

let syncSocket = StormBSP::makeSocket(socket)
syncSocket.read()
syncSocket.write()
syncSocket.close()

addToShareList(clientid, group, peer)
removeFromShareList(client, group, peer)
setMessageValidator(client, function)
setMessageSharingDeterminator(client, function)
setMessageDeletor(client, function)


5. SyncClient -> Platform

expose CLI and API

addToShareList(clientid, group, peer)
removeFromShareList(client, group, peer)
setMessageValidator(client, function)
setMessageSharingDeterminator(client, function)
setMessageDeletor(client, function)

6. Groups -> SyncClient

Groups are just sets of devices, they don't need to use an interface

7. ClientPlugins -> SyncClient

struct intefaceSpecification = {messageType: function, ...}
defineInterface(specification)

8. App -> SyncClient

let group = client.createGroup(options)
client.updateGroup(group, options) // Allow local changes, group-wide if admin. Do the other devices accept the changes? What about bans? Silencing?
client.removeFromGroup(group, device) // Self-remove, blocking is allowed. Kicking if admin. Other clients can choose to accept changes.
client.addMessageToLocalGraph(group, message)

9. App -> ClientPlugins

depends on plugin interface

10. Application

let node = createNode()
let account = createAccount(username, password)
let accData = login(username, password)

let groups = getGroups()
let clients = getClients()

11. Frontent -> Application REST API




node.addContact(pub_key)
