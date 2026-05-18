pub fn system_time_to_unix_ms(time: std::time::SystemTime) -> anyhow::Result<i64> {
    let duration = time.duration_since(std::time::UNIX_EPOCH)?;
    Ok(i64::try_from(duration.as_millis())?)
}
