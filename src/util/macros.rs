#[macro_export]
macro_rules! stage {
    // todo: Export text to global progress text
    ($s:literal $body:block) => {{
        let s = tracing::info_span!($s);

        if let Some(m) = $crate::backend::install::IPC_CHANNEL.get() {
            let sender = m.lock().unwrap();
            // Then we are in a non-interactive install, which means we export IPC
            // to stdout
            let install_status =
                $crate::backend::install::InstallationMessage::Status($s.to_owned());
            sender.send(install_status).unwrap();
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
