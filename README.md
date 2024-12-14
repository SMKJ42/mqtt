#### :warning: Actively in development and NOT a release version. :warning:

#### :warning: The underlying memory for messages with QoS > 0 utilizes a TCP broadcast and WILL lose messages if the queue gets filled. :warning:

## Follows MQTT v3.1.1 Specifications.

The primary goal of this repository is to provide a robust MQTT Broker / Client ecosystem.

### Currently supports:

-   MQTT v3.1.1
-   TLS connections
-   Client initiated QoS level downgrading for Subscribers.
-   In memory Disconnected sessions.
-   Client initiated session cleaning.

### Currently does not Support:

-   MQTT v5
-   MQTT-SN (v1.2)
-   Broker initiated QoS level downgrading.
-   On disk Disconnected sessions.
-   Client authentication.
-   Client authoriziation.
-   Creating topics at program initialization (Client must Pub before other clients can Sub).

### Features:

-   Event logging.
