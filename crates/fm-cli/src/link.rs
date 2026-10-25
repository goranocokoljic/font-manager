//! How `fm` reaches the library (E26.6, ADR 0015): through the running
//! app's local API when it answers, else through `fm-core` on the index.

use std::sync::Arc;

use fm_api::{Api, ApiError, ApiResult, ErrorCode};
use fm_client::Client;
use fm_core::{CoreApi, startup};
use fm_platform::AppPaths;

use crate::Link;

/// The API `fm` runs on, and how it is reached. The running app whose
/// `endpoint.json` is in `paths.data` when it answers ([`endpoint`]); then
/// the index is never opened here, so the app's activation state and
/// events are the ones `fm` sees. Else the index directly ([`index`]).
pub fn reach(paths: &AppPaths, direct: bool) -> ApiResult<(Arc<dyn Api>, Link)> {
    match endpoint(paths, direct) {
        Some(client) => Ok((Arc::new(client), Link::Endpoint)),
        None => Ok((index(paths)?, Link::Direct)),
    }
}

/// The client of the running app, or `None` with `direct` (`--direct`) or
/// when the app is not there: [`fm_client::connect`], so a stale
/// `endpoint.json` (a dead pid, a refused port) is `None` within
/// [`fm_client::CONNECT_TIMEOUT`].
pub fn endpoint(paths: &AppPaths, direct: bool) -> Option<Client> {
    if direct {
        return None;
    }
    fm_client::connect(paths)
}

/// The core over the index in `paths`, with the system's activator.
pub fn index(paths: &AppPaths) -> ApiResult<Arc<dyn Api>> {
    startup::prepare(paths)?;
    let index = startup::open_index(paths)?.index;
    let activator = fm_platform::activator::system(paths).map_err(|e| {
        ApiError::new(ErrorCode::Io, "Font activation is not available.").with_detail(e.to_string())
    })?;
    Ok(Arc::new(CoreApi::open(paths, index, activator)))
}
