# MateriaQ Telemetry

Privacy-preserving telemetry and live-presence API for Spicetify extensions and web dashboards. Zero PII, HMAC-authenticated, backed by Redis.

## Endpoints

| Method | Route | Description | Auth / Headers |
| --- | --- | --- | --- |
| `POST` | `/get-token` | Mint `user_id` + `signature` | `X-Spotify-Id` or `X-App-ID: web` |
| `GET`/`POST` | `/ping` | Record presence (4-min window) | `X-User-ID` + `X-User-Signature` (or cookie) |
| `GET` | `/users` | Aggregated telemetry stats | Public |
| `GET` | `/users/live` | Current live user count | Public |
| `GET` | `/users/{app}` | App-specific stats (`lyrics`, `web`) | Public |
| `GET` | `/healthz` | Liveness health probe | Public |
| `GET` | `/readyz` | Readiness probe (Redis check) | `X-Admin-Token` |


## Configuration

Configure environment variables in `.env`:

### Required

* `REDIS_URL`: Redis / Valkey connection string (e.g. `redis://127.0.0.1:6379`).
* `ADMIN_TOKEN`: Secret administrative token for `/readyz` probe (minimum 12 characters).
* `HMAC_SECRET`: Secret cryptographic key for signing client tokens (minimum 12 characters).

### Optional

* `PORT`: HTTP server listen port (default: `8080`).
* `COOKIE_SECURE`: Sets `Secure` attribute on web authentication cookies (`true`/`false`, default: `true`). Set to `false` for local HTTP development.
* `ENABLE_DOCS`: Serves OpenAPI Swagger documentation at `/docs` (`true`/`false`, default: `false`).
* `IDENTITY_CAP_PER_IP`: Maximum distinct client identities minted per IP per 24 hours (default: `100`).
* `REQUEST_TIMEOUT_SECS`: Global HTTP request timeout duration in seconds (default: `15`).
* `RUST_LOG`: Log filter directives (default: `telemetry=info,tower_http=info,redis=info`).


## License

This project is licensed under the GNU AGPLv3 License. See [LICENCE](./LICENCE) for details.