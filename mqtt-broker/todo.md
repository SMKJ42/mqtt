# TODOs

## Broker Bridging todos

-   Broker only namespace
    [] Broker status namespace - health check
    [] share messages namespace (batch messages? less io)
    [] move topic to new broker namespace

-   General broker namespace
    [] Create internal topic namespace - lookups & connect info & include whitelist

## Authorization todos

[] Create broker role
[] Allow custom roles
[] mTLS (mutual TLS cert auth)
[] Docs for whitelist on topics -- still need to think through best practices for a white list on topics
[] whitelist declaration scheme... should rules be additive? I feel that this will be harder, and more dangerous than declaring the most restrictive rules first.

## CLI tool

[] create user command
[] change user command
[] query user command
[] delete user command
[] delete whitelist rule command
[] create whitelist rule command
[] query whitelist command
[] change whitelist rule command
[] manually create a topic namespace

## Logging

[] log username on connection (next to "connect success: IP")
[] log new subscriptions

## Persistance

[] Store ALL sessions in db as disconnected sessions.
[] When a client publishes to a topic with no listener, insert the message into a db. Delete on first send?

-   ALT use a wrapper on tokio::Sender that holds client usernames, and on tokio::Receiver drop the client username from the Sender wrapper.
-   if expected clients are not connected, persist the message in db. Delete on all clients receipt. this is a lot of overhead tho... maybe a better solution?

## Config

[] Allow topic namespaces on program init
[] Whitelist config (XML, JSON, TOML -- leaning toward JSON for nesting & readability)
