/// IPC installation message for non-interactive mode
#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub enum InstallMessage {
    Status(String),
}

impl InstallMessage {
    pub fn new(s: &str) -> Self {
        Self::Status(s.to_owned())
    }

    pub fn into_json(self) -> String {
        serde_json::to_string(&self).unwrap()
    }
}

#[macro_export]
macro_rules! stage {
    // todo: Export text to global progress text
    ($s:literal $body:block) => {{
        let s = tracing::info_span!($s);

        if std::env::var("NON_INTERACTIVE_INSTALL").is_ok_and(|v| v == "1") {
            // Then we are in a non-interactive install, which means we export IPC
            // to stdout
            let install_status = $crate::util::macros::InstallMessage::new($s);
            println!("{}", install_status.into_json());
        }

        {
            let _guard = s.enter();
            tracing::debug!("Entering stage");
            $body
        }
    }};
}

/// Make an enum and impl Serialize
///
/// # Examples
/// ```rs
/// ini_enum! {
///     pub enum Idk {
///         A,
///         B,
///         C,
///     }
/// }
/// ```
#[macro_export]
macro_rules! ini_enum {
    (@match $field:ident) => {{
        stringify!($field).replace('_', "-").to_lowercase() // We lowercase this because this is systemd style enum
        // todo: probably not the best idea to lowercase this on all enums
    }};
    (@match $field:ident => $s:literal) => {{
        $s.to_string()
    }};
    (
        $(#[$outmeta:meta])*
        $v:vis enum $name:ident {
            $(
                $(#[$meta:meta])?
                $field:ident $(=> $s:literal)?,
            )*$(,)?
        }
    ) => {
        $(#[$outmeta])*
        $v enum $name {$(
            $(#[$meta])?
            $field,
        )*}
        impl serde::Serialize for $name {
            fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
            where
                S: ::serde::Serializer,
            {
                serializer.serialize_str(&match self {$(
                    Self::$field => ini_enum! { @match $field $(=> $s)? },
                )*})
            }
        }
    };
    (
        $(#[$outmeta1:meta])*
        $v1:vis enum $name1:ident {
            $(
                $(#[$meta1:meta])?
                $field1:ident $(=> $s1:literal)?,
            )*$(,)?
        }
        $(
            $(#[$outmeta:meta])*
            $v:vis enum $name:ident {
                $(
                    $(#[$meta:meta])?
                    $field:ident $(=> $s:literal)?,
                )*$(,)?
            }
        )+
    ) => {
        ini_enum! {
            $(
                $(#[$outmeta])*
                $v enum $name {
                    $(
                        $(#[$meta])?
                        $field $(=> $s)?,
                    )*
                }
            )+
        }
        ini_enum! {
            $(#[$outmeta1])*
            $v1 enum $name1 {
                $(
                    $(#[$meta1])?
                    $field1 $(=> $s1)?,
                )*
            }
        }
    }
}
