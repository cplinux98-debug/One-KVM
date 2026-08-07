use super::*;

use crate::atx::{AtxDriverType, AtxState, HddStatus, PowerStatus};
use crate::config::WolTarget;

const WOL_HISTORY_DEFAULT_LIMIT: usize = 5;
const WOL_HISTORY_MAX_LIMIT: usize = 50;

/// ATX state response
///
/// Carries the WOL configuration alongside the ATX hardware state so the console
/// can decide which power controls to offer from a single poll.
#[derive(Serialize)]
pub struct AtxStateResponse {
    pub available: bool,
    pub backend: String,
    pub initialized: bool,
    pub power_status: String,
    pub led_supported: bool,
    pub hdd_status: String,
    pub hdd_supported: bool,
    pub wol_enabled: bool,
    pub wol_targets: Vec<WolTarget>,
    /// True when ATX or WOL is enabled, i.e. the console should show a power button
    pub power_controls_available: bool,
}

impl From<AtxState> for AtxStateResponse {
    fn from(state: AtxState) -> Self {
        Self {
            available: state.available,
            backend: match state.driver {
                AtxDriverType::Gpio => "gpio",
                AtxDriverType::UsbRelay => "usbrelay",
                AtxDriverType::Serial => "serial",
                AtxDriverType::None => "none",
            }
            .to_string(),
            initialized: state.power_configured || state.reset_configured,
            power_status: match state.power_status {
                PowerStatus::On => "on".to_string(),
                PowerStatus::Off => "off".to_string(),
                PowerStatus::Unknown => "unknown".to_string(),
            },
            led_supported: state.led_supported,
            hdd_status: match state.hdd_status {
                HddStatus::Active => "active".to_string(),
                HddStatus::Inactive => "inactive".to_string(),
                HddStatus::Unknown => "unknown".to_string(),
            },
            hdd_supported: state.hdd_supported,
            wol_enabled: false,
            wol_targets: Vec::new(),
            power_controls_available: state.available,
        }
    }
}

impl Default for AtxStateResponse {
    fn default() -> Self {
        Self {
            available: false,
            backend: "none".to_string(),
            initialized: false,
            power_status: "unknown".to_string(),
            led_supported: false,
            hdd_status: "unknown".to_string(),
            hdd_supported: false,
            wol_enabled: false,
            wol_targets: Vec::new(),
            power_controls_available: false,
        }
    }
}

/// Get ATX status
pub async fn atx_status(State(state): State<Arc<AppState>>) -> Result<Json<AtxStateResponse>> {
    let atx_guard = state.atx.read().await;

    let mut response = match atx_guard.as_ref() {
        Some(atx) => AtxStateResponse::from(atx.state().await),
        None => AtxStateResponse::default(),
    };

    let config = state.config.get();
    response.wol_enabled = config.atx.wol_enabled;
    if config.atx.wol_enabled {
        response.wol_targets = config.atx.wol_targets.clone();
    }
    // ATX may be unavailable at runtime (e.g. device missing) even when configured,
    // so OR the live availability with the configured WOL switch.
    response.power_controls_available = response.available || config.atx.power_controls_available();

    Ok(Json(response))
}

/// ATX power control request
#[derive(Deserialize)]
pub struct AtxPowerControlRequest {
    pub action: String, // "short", "long", "reset"
}

/// Control ATX power
pub async fn atx_power(
    State(state): State<Arc<AppState>>,
    Json(req): Json<AtxPowerControlRequest>,
) -> Result<Json<LoginResponse>> {
    let atx_guard = state.atx.read().await;
    let atx = atx_guard
        .as_ref()
        .ok_or_else(|| AppError::Internal("ATX controller not initialized".to_string()))?;

    match req.action.as_str() {
        "short" => {
            atx.power_short().await?;
            Ok(Json(LoginResponse {
                success: true,
                message: Some("Power short press executed".to_string()),
            }))
        }
        "long" => {
            atx.power_long().await?;
            Ok(Json(LoginResponse {
                success: true,
                message: Some("Power long press (force off) executed".to_string()),
            }))
        }
        "reset" => {
            atx.reset().await?;
            Ok(Json(LoginResponse {
                success: true,
                message: Some("Reset button pressed".to_string()),
            }))
        }
        _ => Err(AppError::BadRequest(format!(
            "Unknown ATX action: {}. Valid actions: short, long, reset",
            req.action
        ))),
    }
}

/// WOL request body
#[derive(Debug, Deserialize)]
pub struct WolRequest {
    /// Target MAC address (e.g., "AA:BB:CC:DD:EE:FF" or "AA-BB-CC-DD-EE-FF")
    pub mac_address: String,
}

#[derive(Debug, Deserialize, Default)]
pub struct WolHistoryQuery {
    /// Maximum history entries to return
    pub limit: Option<usize>,
}

#[derive(Debug, Serialize)]
pub struct WolHistoryEntry {
    pub mac_address: String,
    pub updated_at: i64,
}

#[derive(Debug, Serialize)]
pub struct WolHistoryResponse {
    pub history: Vec<WolHistoryEntry>,
}

/// Send Wake-on-LAN magic packet
pub async fn atx_wol(
    State(state): State<Arc<AppState>>,
    Json(req): Json<WolRequest>,
) -> Result<Json<LoginResponse>> {
    let config = state.config.get();

    if !config.atx.wol_enabled {
        return Err(AppError::BadRequest(
            "Wake-on-LAN is disabled in settings".to_string(),
        ));
    }

    let mac_address = crate::atx::normalize_mac_address(&req.mac_address).map_err(|_| {
        AppError::BadRequest(format!("Invalid MAC address: {}", req.mac_address.trim()))
    })?;

    let interface = if config.atx.wol_interface.is_empty() {
        None
    } else {
        Some(config.atx.wol_interface.as_str())
    };

    // Send WOL packet
    crate::atx::send_wol(&mac_address, interface)?;

    if let Err(error) = crate::atx::record_wol_history(state.db.pool(), &mac_address).await {
        warn!("Failed to persist WOL history: {}", error);
    }

    Ok(Json(LoginResponse {
        success: true,
        message: Some(format!("WOL packet sent to {}", mac_address)),
    }))
}

/// Get WOL history
pub async fn atx_wol_history(
    State(state): State<Arc<AppState>>,
    Query(query): Query<WolHistoryQuery>,
) -> Result<Json<WolHistoryResponse>> {
    let limit = query
        .limit
        .unwrap_or(WOL_HISTORY_DEFAULT_LIMIT)
        .clamp(1, WOL_HISTORY_MAX_LIMIT);

    let rows = crate::atx::list_wol_history(state.db.pool(), limit).await?;

    let history = rows
        .into_iter()
        .map(|(mac_address, updated_at)| WolHistoryEntry {
            mac_address,
            updated_at,
        })
        .collect();

    Ok(Json(WolHistoryResponse { history }))
}
