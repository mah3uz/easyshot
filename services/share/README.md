# easyshot share service

A Cloudflare Worker (`easyshot-share`, at `share.easyshot.site`) that takes uploads from the app
and hands out links. `PROTOCOL.md` is the contract; `contract/` holds the fixtures the tests read.

What it does:

- Gives out install tokens for a small proof of work (`/v1/challenge`, `/v1/install`). Challenges
  are signed, not stored; the Ledger remembers each used one until it expires, so each buys one
  install. Each address gets five installs a UTC day.
- Takes a share (`POST /v1/shares`), checking in order: token, version, burst limit, type, size,
  days, the install's daily shares and byte-days, and the service-wide storage stop.
- Streams each 25 MiB part through the Worker into an R2 multipart upload, refusing any part whose
  length is not exactly right, then joins the parts on `complete` and checks the first bytes
  against the type. A mismatch deletes the share. A video's poster waits in `poster.upload`;
  `complete` reads and checks it, then writes those same bytes to `poster.jpg`. Once `complete`
  starts, further parts, posters and completes for the share are refused with 409.
- Deletes a share at any point, aborting an unfinished upload and giving its byte-days back.
- Refuses any request with an `Origin` header and sends no CORS headers, so no web page can use it.
- Deletes a finished share's files at its expiry: the Ledger's alarm is set to the earliest expiry,
  deletes the media, poster and staged poster of every share that is due (300 at a time), and sets
  itself again for the next. A failed delete keeps the share and tries again a minute later.
- Serves the share page at `GET /{id}` from the template in `viewer/`: ready, "uploading" or a
  plain "Page not found", from one Ledger call, with a CSP that allows only the template's own
  script and style, by hash. An expired, deleted, unknown or malformed id all get the same 404, so
  a visitor cannot tell a share was ever there.
- Sends a visit to the bare domain (`GET /`) to `https://easyshot.site` with a temporary redirect.

The `Ledger` Durable Object (SQLite) holds every counter: used challenges, installs per hashed
address (an HMAC of the address and the UTC date; raw addresses are never stored), shares and
byte-days per install per day, pending uploads with their R2 upload id and parts, and the live
bytes behind the storage stop. A share counts toward the stop until a day after it expires, even
once its files are deleted. There is one Ledger, and each of its checks and the writes that follow
run before it waits on anything, so two requests cannot both pass a check that only one of them
fits.

Files go to the bucket `easyshot-shares` under `s/1d/`, `s/3d/` or `s/7d/` and are served from
`m.easyshot.site`, not through the Worker.

## Run the tests

```sh
just share-test
```

`just share-test` (`npm test` in this folder) type-checks and runs the tests in a local Workers runtime, with a local R2 bucket and
Durable Object. They never reach Cloudflare.

## Run it locally

Put five secrets of at least 32 characters each in `.dev.vars` (ignored by git):

```sh
for k in CHALLENGE_KEY INSTALL_KEY UPLOAD_KEY DELETE_KEY IP_KEY; do
  echo "$k=$(openssl rand -base64 33)"
done > .dev.vars
just share-dev
```

## Set it up on Cloudflare (once)

You need the `easyshot.site` zone in the same account and `npx wrangler login`.

1. Create the bucket:

   ```sh
   npx wrangler r2 bucket create easyshot-shares
   ```

2. Serve it from `m.easyshot.site` (the zone id is on the zone's overview page):

   ```sh
   npx wrangler r2 bucket domain add easyshot-shares --domain m.easyshot.site --zone-id <zone id> --min-tls 1.2
   ```

3. Add the three lifecycle rules. They are a backstop for the Ledger's own deletes: each deletes
   its files after its days and aborts an unfinished upload after a day (an upload token lasts six
   hours):

   ```sh
   npx wrangler r2 bucket lifecycle add easyshot-shares expire-1d s/1d/ --expire-days 1 --abort-multipart-days 1
   npx wrangler r2 bucket lifecycle add easyshot-shares expire-3d s/3d/ --expire-days 3 --abort-multipart-days 1
   npx wrangler r2 bucket lifecycle add easyshot-shares expire-7d s/7d/ --expire-days 7 --abort-multipart-days 1
   ```

4. Set the five secrets. Each is a random string of at least 32 characters; the Worker refuses to
   sign anything with a shorter one. Changing `INSTALL_KEY` signs every app out; changing
   `DELETE_KEY` makes existing delete links stop working.

   ```sh
   for k in CHALLENGE_KEY INSTALL_KEY UPLOAD_KEY DELETE_KEY IP_KEY; do
     openssl rand -base64 33 | npx wrangler secret put "$k"
   done
   ```

5. Deploy. This creates the Worker, its route on `share.easyshot.site` and the Ledger:

   ```sh
   just share-deploy
   ```

   `just share-deploy` runs the tests first and deploys only if they pass. Run it again after
   any change here.

The limits are `vars` in `wrangler.jsonc`; change them there and deploy again. The burst limit
(five creates a minute per install) is the `BURST` rate limit binding; its `namespace_id` must be
unique in the account.
