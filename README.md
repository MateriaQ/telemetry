<div align="center">
  <picture>
    <source media="(prefers-color-scheme: light)" srcset="assets/icon-light.svg">
    <source media="(prefers-color-scheme: dark)" srcset="assets/icon.svg">
    <img alt="MateriaQ Telemetry" src="assets/icon.svg" width="80" height="80">
  </picture>

  <h1>Materia<span style="color: #d0bcfe;">Q</span> Telemetry</h1>

  <p>Privacy-preserving live presence and telemetry backend for Spicetify and web dashboards.</p>

  <p>
    <a href="./LICENCE"><img src="https://img.shields.io/badge/License-AGPLv3-66558E?labelColor=332F38" alt="License: AGPLv3"></a>
    <img src="https://img.shields.io/badge/Language-Rust-D0BCFE?labelColor=332F38&logo=rust&logoColor=white" alt="Language: Rust">
  </p>
</div>

---

### ✦ Endpoints

| Method | Route | Description | Auth |
| --- | --- | --- | --- |
| `POST` | `/get-token` | Mint client credentials | `X-Spotify-Id` / `X-App-ID` |
| `POST` | `/ping` | Record presence (4-min window) | Token / Cookie |
| `GET` | `/users` | Aggregated telemetry metrics | Public |
| `GET` | `/users/live` | Current active user count | Public |
| `GET` | `/users/{app}` | App-filtered telemetry (`lyrics`, `web`) | Public |
| `GET` | `/healthz`, `/readyz` | Liveness & Redis readiness probes | Public / Admin |

### ✦ Environment Variables

| Variable | Default | Description |
| --- | --- | --- |
| `REDIS_URL` | *Required* | Redis or Valkey connection string |
| `HMAC_SECRET` | *Required* | Secret key for signing client tokens |
| `ADMIN_TOKEN` | *Required* | Bearer token for `/readyz` |
| `PORT` | `8080` | HTTP port |
| `ENABLE_DOCS` | `false` | Serve Swagger UI at `/docs` |
| `COOKIE_SECURE`| `true` | Restrict session cookie to HTTPS |

### ✦ License

[GNU AGPLv3](./LICENCE)