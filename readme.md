# URL Shortener Service

This project implements a URL shortener service using Axum and Redis. It provides endpoints to generate short links, redirect to original URLs, and retrieve click statistics.

## Features

* **Short Link Generation:** Create short and memorable links for long URLs.
* **Redirection:** Redirect users from short links to the original destinations.
* **Click Statistics:** Track the number of clicks on each short link (requires authentication).
* **Redis Storage:** Uses Redis to store short links and click data.

## Dependencies

* **Axum:** Web framework for Rust.
* **Redis:** In-memory data store.
* **Serde:** Serialization and deserialization library.
* **Tracing:** Logging and instrumentation library.
* **Rand:** Random number generation.

## Environment Variables

* **REDIS_URL:** Redis connection URL (defaults to `redis://127.0.0.1/`).
* **HOST:** Host for binding the server (defaults to `127.0.0.1`).
* **PORT:** Port for binding the server (defaults to `3000`).
* **AUTH_TOKEN:** Authentication token required to access click statistics.

## Endpoints

* **POST /generate:** Generates a short link. Requires an `Authorization` header with the `AUTH_TOKEN`.
* **GET /:short_key:** Redirects to the original URL associated with the short key.
* **GET /:short_key/stats:** Retrieves click statistics for the short key. Requires a `token` query parameter matching the one generated during link creation.

## Running the Service

1. Install Rust and Cargo.
2. Clone this repository.
3. Set the required environment variables.
4. Run `cargo run` to start the server.
