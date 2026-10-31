use serde::Serialize;

#[derive(Debug, PartialEq, Eq, Serialize)]
pub struct AppMetadata {
    pub name: &'static str,
    pub version: &'static str,
    pub platform: &'static str,
    pub milestone: &'static str,
}

pub fn read() -> AppMetadata {
    AppMetadata {
        name: "ECHELON OS",
        version: env!("CARGO_PKG_VERSION"),
        platform: std::env::consts::OS,
        milestone: "M1-A",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn repeated_reads_return_the_same_build_metadata() {
        assert_eq!(read(), read());
        assert_eq!(read().version, "0.1.0");
    }
}
