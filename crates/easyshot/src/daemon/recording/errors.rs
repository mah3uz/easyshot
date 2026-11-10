use es_app::errors::Error;

pub fn io_error(e: &es_record::Error) -> Error {
    match e {
        es_record::Error::Unsupported => Error::Unsupported,
        es_record::Error::AudioDeviceMissing => Error::AudioDeviceMissing,
        es_record::Error::AreaOffMonitor => Error::AreaOffMonitor,
        other => Error::Protocol(other.to_string()),
    }
}

pub fn busy() -> Error {
    Error::Internal("the recording is busy".into())
}

pub fn none_running() -> Error {
    Error::NotFound("a running recording".into())
}
