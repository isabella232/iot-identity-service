// Copyright (c) Microsoft. All rights reserved.

#![deny(rust_2018_idioms)]
#![warn(clippy::all, clippy::pedantic)]

#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
pub struct Settings {
    pub hostname: String,

    pub homedir: std::path::PathBuf,

    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub principal: Vec<Principal>,

    pub provisioning: Provisioning,

    /// Only configurable in debug builds for the sake of tests.
    #[serde(default, skip_serializing)]
    #[cfg_attr(not(debug_assertions), serde(skip_deserializing))]
    pub endpoints: Endpoints,

    pub localid: Option<LocalId>,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub struct Principal {
    pub uid: Credentials,

    pub name: aziot_identity_common::ModuleId,

    #[serde(rename = "idtype")]
    pub id_type: Option<Vec<aziot_identity_common::IdType>>,

    /// Options for this principal's local identity.
    pub localid: Option<LocalIdOpts>,
}

#[derive(
    Clone, Copy, Debug, Eq, Hash, Ord, PartialOrd, PartialEq, serde::Deserialize, serde::Serialize,
)]
pub struct Uid(pub libc::uid_t);

pub type Credentials = Uid;

/// Global options for all local identities.
#[derive(Clone, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
pub struct LocalId {
    /// Identifier for a group of local identity certificates, suffixed to the common name.
    pub domain: String,
}

/// Options for a single local identity.
#[derive(Clone, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(tag = "type")]
pub enum LocalIdOpts {
    /// Options valid when local identities are X.509 credentials. Currently the only
    /// supported credential type, but may change in the future.
    #[serde(rename = "x509")]
    X509 {
        /// Whether the X.509 certificate is a TLS client or server certificate.
        #[serde(default)]
        attributes: aziot_identity_common::LocalIdAttr,
    },
}

#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
#[serde(tag = "method")]
#[serde(rename_all = "lowercase")]
pub enum ManualAuthMethod {
    #[serde(rename = "sas")]
    SharedPrivateKey { device_id_pk: String },
    X509 {
        identity_cert: String,
        identity_pk: String,
    },
}

#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
#[serde(tag = "method")]
#[serde(rename_all = "lowercase")]
pub enum DpsAttestationMethod {
    #[serde(rename = "symmetric_key")]
    SymmetricKey {
        registration_id: String,
        symmetric_key: String,
    },
    X509 {
        registration_id: String,
        identity_cert: String,
        identity_pk: String,
    },
    Tpm {
        registration_id: String,
    },
}

#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub struct Provisioning {
    // This type used to have the `provisioning` field before the `dynamic_reprovisioning` field. It doesn't matter much except that the fields are
    // serialized in the order of definition when generating a new config via `aziot init`, and it would've been nice to serialize
    // the `provisioning` value before the `dynamic_reprovisioning` value.
    //
    // Unfortunately the TOML serializer needs "values" (like `dynamic_reprovisioning`) to come before "tables" (like `provisioning`),
    // otherwise it fails to serialize the value. It ought to not matter for this type because the `provisioning` value is flattened,
    // but the TOML serializer doesn't know this.
    //
    // So we have to move the `dynamic_reprovisioning` field before the `provisioning` field.
    #[serde(default)]
    pub dynamic_reprovisioning: bool,

    #[serde(flatten)]
    pub provisioning: ProvisioningType,
}

#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
#[serde(tag = "source")]
#[serde(rename_all = "lowercase")]
pub enum ProvisioningType {
    Manual {
        iothub_hostname: String,
        device_id: String,
        authentication: ManualAuthMethod,
    },
    Dps {
        global_endpoint: String,
        scope_id: String,
        attestation: DpsAttestationMethod,
    },

    /// Disables provisioning with IoT Hub for devices that use local identities only.
    None,
}

#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
pub struct Endpoints {
    pub aziot_certd: http_common::Connector,
    pub aziot_identityd: http_common::Connector,
    pub aziot_keyd: http_common::Connector,
    pub aziot_tpmd: http_common::Connector,
}

impl Default for Endpoints {
    fn default() -> Self {
        Endpoints {
            aziot_certd: http_common::Connector::Unix {
                socket_path: std::path::Path::new("/run/aziot/certd.sock").into(),
            },
            aziot_identityd: http_common::Connector::Unix {
                socket_path: std::path::Path::new("/run/aziot/identityd.sock").into(),
            },
            aziot_keyd: http_common::Connector::Unix {
                socket_path: std::path::Path::new("/run/aziot/keyd.sock").into(),
            },
            aziot_tpmd: http_common::Connector::Unix {
                socket_path: std::path::Path::new("/run/aziot/tpmd.sock").into(),
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{DpsAttestationMethod, ManualAuthMethod, ProvisioningType, Settings};

    fn load_settings(
        filename: impl AsRef<std::path::Path>,
    ) -> Result<Settings, Box<dyn std::error::Error>> {
        let settings = std::fs::read_to_string(filename)?;
        let settings = toml::from_str(&settings)?;
        Ok(settings)
    }

    #[test]
    fn manual_sas_provisioning_settings_succeeds() {
        let s = load_settings("test/good_sas_config.toml").unwrap();

        assert_eq!(s.provisioning.dynamic_reprovisioning, false);

        match s.provisioning.provisioning {
            ProvisioningType::Manual {
                iothub_hostname: _,
                device_id: _,
                authentication,
            } => match authentication {
                ManualAuthMethod::SharedPrivateKey { device_id_pk: _ } => {}
                _ => panic!("incorrect provisioning type selected"),
            },
            _ => panic!("incorrect provisioning type selected"),
        };
    }

    #[test]
    fn manual_dps_provisioning_settings_succeeds() {
        let s = load_settings("test/good_dps_config.toml").unwrap();

        match s.provisioning.provisioning {
            ProvisioningType::Dps {
                global_endpoint: _,
                scope_id: _,
                attestation,
            } => match attestation {
                DpsAttestationMethod::SymmetricKey {
                    registration_id: _,
                    symmetric_key: _,
                } => (),
                _ => panic!("incorrect provisioning type selected"),
            },
            _ => panic!("incorrect provisioning type selected"),
        };
    }

    #[test]
    fn bad_provisioning_settings_fails() {
        assert!(
            load_settings("test/bad_config.toml").is_err(),
            "provisioning settings read should fail"
        );
    }

    #[test]
    fn bad_local_id_settings_fails() {
        assert!(
            load_settings("test/bad_local_config.toml").is_err(),
            "bad_local_config.toml read should fail"
        );
    }
}
