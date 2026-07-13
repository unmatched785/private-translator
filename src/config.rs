use std::{
    collections::HashSet,
    env, fs,
    net::{IpAddr, SocketAddr},
    path::PathBuf,
};

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AppConfig {
    pub profile: String,
    pub display_name: String,
    pub bind: String,
    pub default_model: String,
    pub auto_open: bool,
    pub max_text_chars: usize,
    #[serde(default)]
    pub local_engine: Option<LocalEngineConfig>,
    pub models: Vec<ModelConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LocalEngineConfig {
    pub model_id: String,
    pub runtime_id: String,
    pub executable: String,
    pub model_path: String,
    #[serde(default = "default_context_size")]
    pub context_size: usize,
    #[serde(default)]
    pub threads: usize,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ModelConfig {
    pub id: String,
    pub label: String,
    pub description: String,
    pub endpoint: String,
    pub api_model: String,
    pub family: ModelFamily,
    pub privacy: PrivacyBoundary,
    pub temperature: f32,
    pub top_p: f32,
    #[serde(default = "default_top_k")]
    pub top_k: i32,
    #[serde(default = "default_repeat_penalty")]
    pub repeat_penalty: f32,
    #[serde(default = "default_context_tokens")]
    pub context_tokens: usize,
    #[serde(default = "default_max_output_tokens")]
    pub max_output_tokens: usize,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub api_key_env: Option<String>,
    #[serde(skip)]
    pub runtime_api_key: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ModelFamily {
    HyMt2,
    Translategemma,
    Mock,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PrivacyBoundary {
    Device,
    PrivateNetwork,
}

#[derive(Clone)]
pub struct LaunchOptions {
    pub config: AppConfig,
    pub open_browser: bool,
}

impl LaunchOptions {
    pub fn from_env_and_args() -> Result<Self> {
        let mut profile = env::var("TRANSLATOR_PROFILE").unwrap_or_else(|_| inferred_profile());
        let mut config_path = env::var_os("TRANSLATOR_CONFIG").map(PathBuf::from);
        let mut open_override = None;

        let mut args = env::args().skip(1);
        while let Some(arg) = args.next() {
            match arg.as_str() {
                "--profile" => {
                    profile = args.next().context("--profile requires a name")?;
                }
                "--config" => {
                    config_path = Some(PathBuf::from(
                        args.next().context("--config requires a file path")?,
                    ));
                }
                "--open" => open_override = Some(true),
                "--no-open" => open_override = Some(false),
                "--help" | "-h" => {
                    println!(
                        "Private Translator\n\n  setup [--portable] [--force] [--from <gguf>]\n  model install|status|verify|path\n\n  --profile lite|quality|demo\n  --config <file>\n  --open | --no-open"
                    );
                    std::process::exit(0);
                }
                other => bail!("Unknown option: {other}"),
            }
        }

        let raw = if let Some(path) = config_path {
            fs::read_to_string(&path).with_context(|| {
                format!("Could not read the configuration file: {}", path.display())
            })?
        } else {
            embedded_profile(&profile)?.to_owned()
        };

        let mut config: AppConfig =
            serde_json::from_str(&raw).context("The configuration JSON is invalid")?;
        config.validate()?;
        resolve_runtime_api_keys(&mut config, |variable| env::var(variable).ok());
        let open_browser = open_override.unwrap_or(config.auto_open);

        Ok(Self {
            config,
            open_browser,
        })
    }
}

fn resolve_runtime_api_keys(
    config: &mut AppConfig,
    mut read_secret: impl FnMut(&str) -> Option<String>,
) {
    for model in &mut config.models {
        model.runtime_api_key = model
            .api_key_env
            .as_deref()
            .and_then(&mut read_secret)
            .filter(|secret| !secret.trim().is_empty());
    }
}

fn inferred_profile() -> String {
    env::current_exe()
        .ok()
        .and_then(|path| {
            path.file_stem()
                .map(|name| name.to_string_lossy().into_owned())
        })
        .map(|name| profile_from_executable_name(&name))
        .unwrap_or("lite")
        .into()
}

fn profile_from_executable_name(name: &str) -> &'static str {
    if name.to_ascii_lowercase().contains("quality") {
        "quality"
    } else {
        "lite"
    }
}

impl AppConfig {
    pub fn validate(&self) -> Result<()> {
        if self.profile.trim().is_empty() || self.display_name.trim().is_empty() {
            bail!("The profile and display name cannot be empty");
        }
        if !(1..=1_000_000).contains(&self.max_text_chars) {
            bail!("max_text_chars must be between 1 and 1000000");
        }
        if self.models.is_empty() {
            bail!("At least one translation model is required");
        }
        if !self
            .models
            .iter()
            .any(|model| model.id == self.default_model)
        {
            bail!(
                "default_model is not present in models: {}",
                self.default_model
            );
        }
        let bind = self
            .bind
            .parse::<SocketAddr>()
            .context("The application bind address is invalid")?;
        if !bind.ip().is_loopback() {
            bail!("The application may bind only to a loopback address");
        }
        if bind.port() == 80 {
            bail!(
                "The application cannot use HTTP's default port because exact Host validation requires an explicit port"
            );
        }
        let mut model_ids = HashSet::with_capacity(self.models.len());
        for model in &self.models {
            if model.id.trim().is_empty() {
                bail!("Translation model IDs cannot be empty");
            }
            if model.label.trim().is_empty() || model.api_model.trim().is_empty() {
                bail!(
                    "Translation model labels and API model IDs cannot be empty: {}",
                    model.id
                );
            }
            if !model_ids.insert(model.id.as_str()) {
                bail!("Translation model IDs must be unique: {}", model.id);
            }
            if let Some(variable) = &model.api_key_env
                && (variable.is_empty()
                    || !variable
                        .bytes()
                        .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_'))
            {
                bail!(
                    "Model api_key_env must be an environment variable name: {}",
                    model.id
                );
            }
            validate_model_endpoint(model)?;
            if !model.temperature.is_finite() || !(0.0..=2.0).contains(&model.temperature) {
                bail!("Model temperature is outside the safe range: {}", model.id);
            }
            if !(model.top_p.is_finite() && 0.0 < model.top_p && model.top_p <= 1.0) {
                bail!("Model top_p is outside the safe range: {}", model.id);
            }
            if model.top_k < -1 || model.top_k > 10_000 {
                bail!("Model top_k is outside the safe range: {}", model.id);
            }
            if !(0.0..=2.0).contains(&model.repeat_penalty) {
                bail!(
                    "Model repeat_penalty is outside the safe range: {}",
                    model.id
                );
            }
            if model.context_tokens < 512 || model.context_tokens > 131_072 {
                bail!(
                    "Model context_tokens is outside the safe range: {}",
                    model.id
                );
            }
            if model.max_output_tokens < 64
                || model.max_output_tokens >= model.context_tokens.saturating_sub(128)
            {
                bail!(
                    "Model max_output_tokens is invalid for context_tokens: {}",
                    model.id
                );
            }
        }
        if let Some(engine) = &self.local_engine {
            let model = self.model(&engine.model_id).with_context(|| {
                format!(
                    "The local_engine model is not present in models: {}",
                    engine.model_id
                )
            })?;
            if model.family == ModelFamily::Mock || model.privacy != PrivacyBoundary::Device {
                bail!("local_engine must be a real model running on this device");
            }
            if engine.executable.trim().is_empty() || engine.model_path.trim().is_empty() {
                bail!("local_engine requires an executable and model path");
            }
            if engine.runtime_id.trim().is_empty() {
                bail!("local_engine requires runtime_id");
            }
            if engine.context_size < 512 || engine.context_size > 131_072 {
                bail!("local_engine context_size is outside the safe range");
            }
            if engine.context_size != model.context_tokens {
                bail!("local_engine context_size must match model context_tokens");
            }
            if engine.threads > 256 {
                bail!("local_engine threads is outside the safe range");
            }
        }
        Ok(())
    }

    pub fn model(&self, id: &str) -> Option<&ModelConfig> {
        self.models.iter().find(|model| model.id == id)
    }
}

fn default_context_size() -> usize {
    4096
}

fn default_top_k() -> i32 {
    -1
}

fn default_repeat_penalty() -> f32 {
    1.0
}

fn default_context_tokens() -> usize {
    4096
}

fn default_max_output_tokens() -> usize {
    2048
}

fn validate_model_endpoint(model: &ModelConfig) -> Result<()> {
    if model.privacy == PrivacyBoundary::PrivateNetwork && model.api_key_env.is_none() {
        bail!(
            "A private_network model requires api_key_env authentication: {}",
            model.id
        );
    }
    if model.family == ModelFamily::Mock {
        return Ok(());
    }
    let url = reqwest::Url::parse(&model.endpoint)
        .with_context(|| format!("The model endpoint is invalid: {}", model.id))?;
    if !matches!(url.scheme(), "http" | "https") {
        bail!("The model endpoint must use http or https: {}", model.id);
    }
    let host = url
        .host_str()
        .with_context(|| format!("The model endpoint has no host: {}", model.id))?;
    if !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
    {
        bail!(
            "The model endpoint cannot contain credentials, a query, or a fragment: {}",
            model.id
        );
    }

    match model.privacy {
        PrivacyBoundary::Device if !is_loopback_host(host) => {
            bail!(
                "A device model may use only an address on this device: {}",
                model.id
            )
        }
        PrivacyBoundary::PrivateNetwork if !is_private_host(host) => {
            bail!(
                "A private_network model must use a private IP address literal: {}",
                model.id
            )
        }
        PrivacyBoundary::PrivateNetwork
            if !is_loopback_host(host) && !url.scheme().eq_ignore_ascii_case("https") =>
        {
            bail!(
                "A non-loopback private_network model must use HTTPS: {}",
                model.id
            )
        }
        _ => Ok(()),
    }
}

fn is_loopback_host(host: &str) -> bool {
    host.eq_ignore_ascii_case("localhost")
        || host
            .parse::<IpAddr>()
            .is_ok_and(|address| address.is_loopback())
}

fn is_private_host(host: &str) -> bool {
    if is_loopback_host(host) {
        return true;
    }
    host.parse::<IpAddr>().is_ok_and(|address| match address {
        IpAddr::V4(address) => {
            let [first, second, _, _] = address.octets();
            address.is_private()
                || address.is_link_local()
                || (first == 100 && (64..=127).contains(&second))
        }
        IpAddr::V6(address) => {
            address.is_unique_local() || address.is_unicast_link_local() || address.is_loopback()
        }
    })
}

fn embedded_profile(profile: &str) -> Result<&'static str> {
    match profile {
        "lite" => Ok(include_str!("../configs/lite.json")),
        "quality" => Ok(include_str!("../configs/quality.json")),
        "demo" => Ok(include_str!("../configs/demo.json")),
        _ => bail!("Unsupported profile: {profile}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn embedded_profiles_are_private_and_valid() {
        for profile in ["lite", "quality", "demo"] {
            let config: AppConfig =
                serde_json::from_str(embedded_profile(profile).unwrap()).unwrap();
            config.validate().unwrap();
        }
    }

    #[test]
    fn external_model_endpoint_is_rejected() {
        let mut config: AppConfig =
            serde_json::from_str(embedded_profile("lite").unwrap()).unwrap();
        config.models[0].endpoint = "https://external.example/v1".into();
        assert!(config.validate().is_err());
    }

    #[test]
    fn duplicate_model_ids_are_rejected() {
        let mut config: AppConfig =
            serde_json::from_str(embedded_profile("quality").unwrap()).unwrap();
        config.models[1].id = config.models[0].id.clone();
        assert!(config.validate().is_err());
    }

    #[test]
    fn private_network_hostnames_and_plaintext_lan_are_rejected() {
        let mut config: AppConfig =
            serde_json::from_str(embedded_profile("quality").unwrap()).unwrap();
        config.models[1].endpoint = "https://translator-box/v1".into();
        assert!(config.validate().is_err());

        config.models[1].endpoint = "http://192.168.1.20/v1".into();
        assert!(config.validate().is_err());

        config.models[1].endpoint = "https://192.168.1.20/v1".into();
        assert!(config.validate().is_ok());
    }

    #[test]
    fn private_network_loopback_requires_auth_but_managed_device_does_not() {
        let mut quality: AppConfig =
            serde_json::from_str(embedded_profile("quality").unwrap()).unwrap();
        assert_eq!(
            quality.models[1].api_key_env.as_deref(),
            Some("PRIVATE_TRANSLATOR_QUALITY_API_KEY")
        );

        quality.models[1].api_key_env = None;
        let error = quality.validate().unwrap_err().to_string();
        assert!(error.contains("private_network model requires api_key_env"));

        let lite: AppConfig = serde_json::from_str(embedded_profile("lite").unwrap()).unwrap();
        assert_eq!(lite.models[0].privacy, PrivacyBoundary::Device);
        assert!(lite.models[0].api_key_env.is_none());
        assert!(lite.validate().is_ok());
    }

    #[test]
    fn missing_optional_quality_secret_leaves_only_that_runtime_key_unresolved() {
        let mut quality: AppConfig =
            serde_json::from_str(embedded_profile("quality").unwrap()).unwrap();

        resolve_runtime_api_keys(&mut quality, |_| None);

        assert!(quality.validate().is_ok());
        assert!(quality.models[0].runtime_api_key.is_none());
        assert!(quality.models[1].runtime_api_key.is_none());

        resolve_runtime_api_keys(&mut quality, |variable| {
            (variable == "PRIVATE_TRANSLATOR_QUALITY_API_KEY").then(|| "quality-secret".into())
        });
        assert_eq!(
            quality.models[1].runtime_api_key.as_deref(),
            Some("quality-secret")
        );
    }

    #[test]
    fn unusable_text_and_sampling_limits_are_rejected() {
        let mut config: AppConfig =
            serde_json::from_str(embedded_profile("lite").unwrap()).unwrap();
        config.max_text_chars = 0;
        assert!(config.validate().is_err());

        config.max_text_chars = 50_000;
        config.models[0].temperature = -0.1;
        assert!(config.validate().is_err());

        config.models[0].temperature = 0.7;
        config.models[0].top_p = 1.1;
        assert!(config.validate().is_err());

        config.models[0].top_p = 0.6;
        config.models[0].api_model.clear();
        assert!(config.validate().is_err());

        config.models[0].api_model = "hy-mt2-1.8b".into();
        config.models[0].max_output_tokens = usize::MAX;
        assert!(config.validate().is_err());
    }

    #[test]
    fn unknown_configuration_fields_are_rejected() {
        let raw = embedded_profile("lite").unwrap().replace(
            "\"auto_open\": true",
            "\"auto_open\": true, \"auto_opne\": true",
        );
        assert!(serde_json::from_str::<AppConfig>(&raw).is_err());
    }

    #[test]
    fn quality_executable_name_selects_quality_profile() {
        assert_eq!(
            profile_from_executable_name("PrivateTranslator-Quality"),
            "quality"
        );
        assert_eq!(
            profile_from_executable_name("PrivateTranslator-Lite"),
            "lite"
        );
    }
}
