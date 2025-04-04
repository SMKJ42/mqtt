If you're crazy enough to want to help out, just submit a pull request. Here are some high level things that need work. Others are contained in the subdirectories.

### The road to MQTT world domination.

    [X] MQTT v3.1.1 packet parsing library.
    [X] MQTT v3.1.1 basic broker implementation.
    [X] Full MQTT v3.1.1 compliance.
    [] Optimize MQTT v3.1.1 implementation.
    [] MQTT v5 packet parsing library.
    [] MQTT v5 broker implementation.
    [] Full MQTT v5 compliance.
    [] Optimize MQTT v5 implementation.
    [X] Non-embedable Client library.
    [] Embeddable Client library.
    [] Sync Client library.
    [X] Client Authentication.
    [] Broker bridging.
    [X] TLS connections.
    [] mTLS connections.
    [X] Server configuration file.

### Debug

    [] Hanging -- I think on TCP stream read buffer fill -- packets get put into unreadable state where the client / broker does not read fast enough...
