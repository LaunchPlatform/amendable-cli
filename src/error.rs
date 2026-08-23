use std::fmt;
use std::io;
use std::process::ExitCode;

#[derive(Debug)]
pub enum Error {
    Api { status: u16, detail: String },
    NotLoggedIn,
    Message(String),
    Io(io::Error),
    Http(reqwest::Error),
    Json(serde_json::Error),
    TomlDe(toml::de::Error),
    TomlSer(toml::ser::Error),
    Git(i32),
    Aborted,
}

impl Error {
    pub fn api(status: u16, detail: impl Into<String>) -> Self {
        Self::Api {
            status,
            detail: detail.into(),
        }
    }

    pub fn message(msg: impl Into<String>) -> Self {
        Self::Message(msg.into())
    }

    pub fn exit_code(&self) -> u8 {
        match self {
            Self::Git(code) => (*code).clamp(1, 255) as u8,
            _ => 1,
        }
    }

    pub fn print(&self) {
        match self {
            Self::Api { status, detail } => eprintln!("{status} {detail}"),
            Self::NotLoggedIn => {
                eprintln!("Not logged in. Run `amendable login` or set AMENDABLE_TOKEN.");
            }
            Self::Aborted => eprintln!("Aborted."),
            Self::Git(_) => {}
            other => eprintln!("{other}"),
        }
    }

    pub fn exit(self) -> ExitCode {
        self.print();
        ExitCode::from(self.exit_code())
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Api { status, detail } => write!(f, "{status}: {detail}"),
            Self::NotLoggedIn => {
                write!(
                    f,
                    "Not logged in. Run `amendable login` or set AMENDABLE_TOKEN."
                )
            }
            Self::Message(msg) => write!(f, "{msg}"),
            Self::Io(err) => write!(f, "{err}"),
            Self::Http(err) => write!(f, "{err}"),
            Self::Json(err) => write!(f, "{err}"),
            Self::TomlDe(err) => write!(f, "{err}"),
            Self::TomlSer(err) => write!(f, "{err}"),
            Self::Git(code) => write!(f, "git exited with status {code}"),
            Self::Aborted => write!(f, "Aborted."),
        }
    }
}

impl std::error::Error for Error {}

impl From<io::Error> for Error {
    fn from(err: io::Error) -> Self {
        Self::Io(err)
    }
}

impl From<reqwest::Error> for Error {
    fn from(err: reqwest::Error) -> Self {
        Self::Http(err)
    }
}

impl From<serde_json::Error> for Error {
    fn from(err: serde_json::Error) -> Self {
        Self::Json(err)
    }
}

impl From<toml::de::Error> for Error {
    fn from(err: toml::de::Error) -> Self {
        Self::TomlDe(err)
    }
}

impl From<toml::ser::Error> for Error {
    fn from(err: toml::ser::Error) -> Self {
        Self::TomlSer(err)
    }
}
