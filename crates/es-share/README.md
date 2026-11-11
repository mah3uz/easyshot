# es-share

The client for the hosted share service: install token by proof-of-work, create, part upload with progress, retry and cancel, complete and delete, over blocking HTTPS (`ureq` with rustls) on a job thread. The own-bucket target: path-style S3 uploads to the user's bucket, signed with its own AWS Signature Version 4 signer, with progress, cancel, delete and a connection test. Also the share page template (`viewer::PAGE`) and its filler, identical to the service's. Writes no files: the caller stores the install token and share records through es-store.

May depend on: es-model.

Must not depend on: gpui*, es-ui, tokio, any layer-3 crate; it never writes a file.
