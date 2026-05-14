use zstash_core::ipc::v1::commands::version::{GetVersionRequest, GetVersionResponse};
use zstash_core::ipc::v1::common::{IpcResult, SCHEMA_VERSION, ensure_schema_version};
use zstash_core::version::VersionInfo;

#[tauri::command]
pub fn zstash_get_version(request: GetVersionRequest) -> IpcResult<GetVersionResponse> {
    if let Err(err) = ensure_schema_version(request.schema_version) {
        return IpcResult::Err { err };
    }

    IpcResult::ok(GetVersionResponse {
        schema_version: SCHEMA_VERSION,
        version_info: VersionInfo::current(),
    })
}
