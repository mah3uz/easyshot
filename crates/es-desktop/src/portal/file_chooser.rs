use super::portal_error;
use crate::bus::connect;
use crate::dbus::Endpoint;
use crate::{Error, FilePicker, OpenRequest, Result, SaveRequest};
use ashpd::desktop::file_chooser::SelectedFiles;
use futures::future::BoxFuture;
use std::path::PathBuf;

pub struct PortalFilePicker {
    endpoint: Endpoint,
}

impl PortalFilePicker {
    pub fn new(endpoint: Endpoint) -> Self {
        PortalFilePicker { endpoint }
    }
}

fn uri_to_path(uri: &str) -> Option<PathBuf> {
    use std::os::unix::ffi::OsStringExt;
    let rest = uri.strip_prefix("file://")?;
    let rest = rest.strip_prefix("localhost").unwrap_or(rest);
    let bytes = rest.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            let hex = std::str::from_utf8(&bytes[i + 1..i + 3]).ok()?;
            out.push(u8::from_str_radix(hex, 16).ok()?);
            i += 3;
        } else {
            out.push(bytes[i]);
            i += 1;
        }
    }
    Some(PathBuf::from(std::ffi::OsString::from_vec(out)))
}

fn paths(files: &SelectedFiles) -> Vec<PathBuf> {
    files
        .uris()
        .iter()
        .filter_map(|u| uri_to_path(u.as_str()))
        .collect()
}

impl FilePicker for PortalFilePicker {
    fn save(&self, req: SaveRequest) -> BoxFuture<'static, Result<Option<PathBuf>>> {
        let endpoint = self.endpoint.clone();
        Box::pin(async move {
            let conn = connect(&endpoint).await?;
            let request = SelectedFiles::save_file()
                .title(req.title.as_str())
                .current_name(req.suggested_name.as_str())
                .current_folder::<PathBuf>(req.dir.clone())
                .map_err(portal_error)?
                .connection(Some(conn))
                .send()
                .await
                .map_err(portal_error)?;
            match request.response().map_err(portal_error) {
                Ok(files) => Ok(paths(&files).into_iter().next()),
                Err(Error::Cancelled) => Ok(None),
                Err(e) => Err(e),
            }
        })
    }

    fn open(&self, req: OpenRequest) -> BoxFuture<'static, Result<Vec<PathBuf>>> {
        let endpoint = self.endpoint.clone();
        Box::pin(async move {
            let conn = connect(&endpoint).await?;
            let request = SelectedFiles::open_file()
                .title(req.title.as_str())
                .multiple(req.multiple)
                .current_folder::<PathBuf>(req.dir.clone())
                .map_err(portal_error)?
                .connection(Some(conn))
                .send()
                .await
                .map_err(portal_error)?;
            match request.response().map_err(portal_error) {
                Ok(files) => Ok(paths(&files)),
                Err(Error::Cancelled) => Ok(Vec::new()),
                Err(e) => Err(e),
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::uri_to_path;
    use std::path::PathBuf;

    #[test]
    fn uris_decode_to_paths() {
        assert_eq!(
            uri_to_path("file:///home/a/My%20Shot%25.png"),
            Some(PathBuf::from("/home/a/My Shot%.png"))
        );
        assert_eq!(uri_to_path("https://x"), None);
    }
}
