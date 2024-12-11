:warning: Actively in development and NOT a release version. :warning:

:warning: The underlying memory for messages with QoS > 0 utilizes a TCP broadcast and WILL lose messages if the queue gets filled. :warning:

## Follows MQTT v3.1.1 Specifications.

The primary goal of this repository is to provide a robust MQTT Broker / Client ecosystem.

:warning: Before using the TLS feature :warning:

-   Enter the `mqtt-broker/openssl.cnf` file and update the information to generate your TLS certificate
-   If you want to modify the cert script it can be found at `mqtt-broker/src/init fn init_tls_cert()`

### Currently supports:

-   MQTT V3.1.1
-   TLS connections
-   Client initiated QoS level downgrading for Subscribers.
-   In memory Disconnected sessions.
-   Client initiated session cleaning.

### Currently does not Support:

-   MQTT V5
-   MQTT-SN (V1.2)
-   Broker initiated QoS level downgrading.
-   On disk Disconnected sessions.
-   Client authentication.
-   Client authoriziation.
-   Creating topics at program initialization (Client must Pub before other clients can Sub).

### Features:

-   Event logging.
