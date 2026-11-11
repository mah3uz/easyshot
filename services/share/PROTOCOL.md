# easyshot share protocol, v1

The contract between the app (`crates/es-share`) and the share service (this directory). The
fixtures in `contract/` are read by the tests on both sides; a change here changes them in the
same commit.

## Hosts

| Host | Serves |
|---|---|
| `https://share.easyshot.site/v1/…` | the API (this Worker) |
| `https://share.easyshot.site/{id}` | the share page (this Worker) |
| `https://m.easyshot.site/s/{d}d/{id}/…` | the files (public R2 bucket on its custom domain) |

## Rules for every `/v1` request

- JSON bodies, UTF-8. Times are RFC 3339 in UTC with a `Z` (`2026-11-18T10:00:00Z`).
- The app sends `X-Easyshot-Version: <semver>`. Missing or below the service's minimum: `426
  version_unsupported`.
- A request carrying an `Origin` header is refused with `403 origin_refused`. The service sends no
  CORS headers, so no web page can upload.
- Errors are `{"error": {"code": "<code>", "message": "<English, for logs>", "retry_after_s": N}}`;
  `retry_after_s` only where noted. The app shows its own text for each code, never `message`.

| Code | Status | When |
|---|---|---|
| `bad_request` | 400 | malformed body or parameters |
| `unauthorized` | 401 | token missing, malformed or not valid for this request |
| `token_expired` | 401 | upload token past its time |
| `challenge_failed` | 403 | proof-of-work wrong, or challenge expired or reused |
| `origin_refused` | 403 | an `Origin` header was sent |
| `not_found` | 404 | no such share, or it is gone |
| `conflict` | 409 | a part has the wrong size, or `complete` before every part arrived |
| `too_large` | 413 | over the size cap for the kind |
| `unsupported_type` | 415 | type not allowed, or the bytes are not what `mime` says |
| `version_unsupported` | 426 | app too old |
| `rate_limited` | 429 | burst limit; `retry_after_s` |
| `quota_daily` | 429 | the install's daily shares or byte-days are spent; `retry_after_s` to 00:00 UTC |
| `full` | 503 | the service-wide storage stop; `retry_after_s` |

## Install token

1. `GET /v1/challenge` → `200 {"challenge": "<base64url>", "difficulty": 20, "expires_at": "…"}`.
   The challenge is opaque to the app (the service signs it; no server state).
2. The app finds the smallest decimal `nonce` ≥ 0 such that `SHA-256(challenge + ":" + nonce)`
   (ASCII, no padding, no spaces) starts with at least `difficulty` zero bits.
3. `POST /v1/install` `{"challenge": "…", "nonce": "402"}` → `200 {"install_token": "…"}`.
   A challenge is good for one install and until `expires_at`. Each hashed IP gets at most 5
   installs per UTC day (`rate_limited` beyond).

The install token does not expire. The app keeps it and asks again only after `unauthorized`.

## Limits

`GET /v1/limits`, `Authorization: Bearer <install_token>` optional →

```json
{
  "image_max_bytes": 10485760,
  "gif_max_bytes": 20971520,
  "video_max_bytes": 209715200,
  "poster_max_bytes": 409600,
  "days": [1, 3, 7],
  "default_days": 7,
  "part_bytes": 26214400,
  "daily_shares": 30,
  "daily_byte_days": 1468006400,
  "shares_left": 30,
  "byte_days_left": 1468006400,
  "accepting": true
}
```

`shares_left` and `byte_days_left` are present only with a valid token. `accepting` is false at
the storage stop. The app never hardcodes any of these.

Byte-days: a share costs `size × days` (the media plus the poster). The install's daily budget
and the service's live-bytes stop are counted when the share is created. A share created and
then deleted before `complete` gives its cost back; after `complete` it does not.

## Create, upload, complete

`POST /v1/shares`, `Authorization: Bearer <install_token>`:

```json
{
  "kind": "image",
  "mime": "image/png",
  "size": 1843210,
  "days": 7,
  "title": "Login bug",
  "width": 1920,
  "height": 1080,
  "duration_ms": null,
  "poster_size": null
}
```

- `kind`: `image` (mime `image/png`, `image/jpeg`, `image/webp`, `image/gif`) or `video` (mime
  `video/mp4`). `title` ≤ 120 characters or null. `duration_ms` and `poster_size` are required for
  video and null for images.
- Checked in order: token, version, burst limit, type, size cap, `days` in `days`, daily quota,
  storage stop.

`201` →

```json
{
  "id": "7Kp2mX9qR4tY8wZ1aB3cD5",
  "page_url": "https://share.easyshot.site/7Kp2mX9qR4tY8wZ1aB3cD5",
  "media_url": "https://m.easyshot.site/s/7d/7Kp2mX9qR4tY8wZ1aB3cD5/media.png",
  "poster_url": null,
  "expires_at": "2026-11-18T10:00:00Z",
  "upload_token": "…",
  "upload_expires_at": "2026-11-11T16:00:00Z",
  "part_bytes": 26214400,
  "parts": 1,
  "delete_token": "…"
}
```

- `id`: 22 characters of base64url. The first is the days class (`1`, `3` or `7`); the other 21
  are random (126 bits). `^[137][A-Za-z0-9_-]{21}$`.
- The link works from this moment; the page shows "uploading" until `complete`.
- `parts = ceil(size / part_bytes)`. Every part is `part_bytes` long except the last.
- The upload token is good for 6 hours and only for this share.

`PUT /v1/shares/{id}/parts/{n}` (n from 1), `Authorization: Bearer <upload_token>`, a body of
exactly the part's length → `200 {"part": n}`. Sending a part again replaces it (retry).

`PUT /v1/shares/{id}/poster`, upload token, `image/jpeg`, `poster_size` bytes → `200 {}`. Video
only, before `complete`.

`POST /v1/shares/{id}/complete`, upload token → `200 {"page_url", "media_url", "poster_url",
"expires_at"}`. The service checks every part (and the poster for video) arrived and the first
bytes match `mime` (PNG `89 50 4E 47`; JPEG `FF D8 FF`; GIF `GIF8`; WebP `RIFF????WEBP`; MP4
`ftyp` at offset 4). A mismatch deletes the share: `415 unsupported_type`.

## Delete

`DELETE /v1/shares/{id}`, `Authorization: Bearer <delete_token>` → `204`. Works at any point,
including mid-upload. Deleting a gone share is `404 not_found`; the app treats that as done.

## Storage layout (service)

`s/{d}d/{id}/media.{png|jpg|webp|gif|mp4}` and `s/{d}d/{id}/poster.jpg`, `{d}` in 1, 3, 7, each
prefix with an R2 lifecycle rule deleting after `d` days. A share counts toward the live-bytes stop
until a day after `expires_at` (R2 deletes up to a day late). The share page answers `410` from
`expires_at` on.

## Own bucket (no service)

The app writes to the user's S3-compatible bucket, signed with SigV4:
`{prefix}s/{id}/media.{ext}`, `{prefix}s/{id}/poster.jpg` (video), and `{prefix}s/{id}/index.html`
(the share page, filled as below, `ready` state, no `expires` block). `id` is 22 random base64url
characters. The page link is `{public_base_url}/s/{id}/index.html`; the file link is the media URL.
There is no expiry; the app deletes the three objects on Delete share.

## The share page template

`viewer/viewer.html` is one self-contained file (inline CSS, inline SVG, inline script). The
Worker fills it for Free shares; the app embeds the same file and fills it for own-bucket shares.

Filling, identical on both sides (`contract/viewer.json` holds the cases):

1. Blocks: each `<!--if:NAME-->…<!--end:NAME-->` (NAME `[a-z]+`, not nested) keeps its content,
   without the markers, when NAME is in the given set; otherwise the whole block goes.
2. Then each `{{name}}` (`[a-z_]+`) is replaced by its value, HTML-escaped: `&` `&amp;`, `<` `&lt;`,
   `>` `&gt;`, `"` `&quot;`, `'` `&#39;`. One pass: a value is never expanded again.
3. A placeholder left in the output with no value is the error `missing_value`. A value whose name
   ends in `_url` must start with `https://`, or it is the error `not_https`.
4. Placeholders appear only in HTML text and attribute values, never inside `<script>` or
   `<style>` (HTML escaping does not make a value safe there). The page's script reads what it
   needs from `data-` attributes.

Blocks: `ready`, `uploading`, `gone`, `image`, `video`, `expires`.

| Placeholder | Value |
|---|---|
| `title` | the title, or the default the filler's caller picks |
| `page_url`, `media_url`, `poster_url` | as from the API (`poster_url` is the media URL for an image) |
| `mime`, `width`, `height` | of the media |
| `expires_at` | RFC 3339 (inside the `expires` block only) |
| `download_name` | `easyshot-{id}.{ext}` |
| `home_url` | `https://easyshot.site` |
