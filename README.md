#### :warning: Actively in development and NOT a release version. :warning:

## Follows MQTT v3.1.1 Specifications.

The primary goal of this repository is to provide a robust MQTT Broker / Client ecosystem.

### Currently supports:

-   MQTT v3.1.1
-   TLS connections
-   Client & client initiated QoS level downgrading.
-   In memory Disconnected sessions.
-   Client initiated session cleaning.

### Currently does not Support:

-   MQTT v5
-   MQTT-SN (v1.2)
-   On disk Disconnected sessions.
-   Client authentication.
-   Client authoriziation.
-   Creating topics at program initialization (Client must Pub before other clients can Sub).
-   Broker bridging (I am delaying this until I decide on how to scale the broker network).

### Features:

-   Event logging.
