#### :warning: Actively in development and NOT a release version :warning:

## Getting started

-   For familiarity, a new broker will log to the console. If you want to toggle this feature, see the `config.toml` file in the project's directory.
-   The default address is 127.0.0.1:1883

### :warning: Before using the TLS feature :warning:

-   Enter the `mqtt-broker/openssl.cnf` file and update the information to generate your TLS certificate
-   If you want to modify the cert script it can be found at `mqtt-broker/src/init -> init_tls_cert()`
-   While not required it is recommended to change the port from 1883 (plaintext) to 8883 (TLS). You can change this is the `config.toml` file in the project's directory.
