# TODO's

config.toml file

## Persist Session

Persist active and disconnected and Active Sessions to a database

persistant_sessions: bool

default - true

## Existing topic names at buildtime

topics: String[]

default - []

## Allow topics from PUBLISH packet with non-existing topic

can_client_make_topic: bool

default - true

## Authenticated topic names / filters.

Wildcard matching on config file for securing topics.

auth_routes: String[] -- parsed to TopicFilter

default - []

## MQTT supported versions

versions: String[]

Currently only V3, but when V5 is developed this will be supported to enable multiple version support.

default - [ V3 ]

## Launch CLI on startup

cli: bool

default - true
