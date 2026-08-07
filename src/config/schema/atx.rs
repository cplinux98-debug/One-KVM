use serde::{Deserialize, Serialize};
use typeshare::typeshare;

pub use crate::atx::{ActiveLevel, AtxDriverType, AtxInputBinding, AtxOutputBinding};

/// Maximum number of Wake-on-LAN targets that can be stored in settings.
pub const WOL_TARGET_MAX_COUNT: usize = 5;

/// Maximum length of a Wake-on-LAN target display name.
pub const WOL_TARGET_NAME_MAX_CHARS: usize = 32;

/// A named Wake-on-LAN target configured in settings.
///
/// Console clients pick a target from this list instead of typing a MAC address.
#[typeshare]
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct WolTarget {
    pub name: String,
    pub mac: String,
}

#[typeshare]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct AtxConfig {
    pub enabled: bool,
    pub driver: AtxDriverType,
    pub device: String,
    pub baud_rate: u32,
    pub power: AtxOutputBinding,
    pub reset: AtxOutputBinding,
    pub led: AtxInputBinding,
    pub hdd: AtxInputBinding,
    /// Whether Wake-on-LAN is offered to console clients
    pub wol_enabled: bool,
    pub wol_interface: String,
    /// Named Wake-on-LAN targets, at most [`WOL_TARGET_MAX_COUNT`] entries
    pub wol_targets: Vec<WolTarget>,
}

impl Default for AtxConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            driver: AtxDriverType::None,
            device: String::new(),
            baud_rate: 9600,
            power: AtxOutputBinding::default(),
            reset: AtxOutputBinding::default(),
            led: AtxInputBinding::default(),
            hdd: AtxInputBinding::default(),
            wol_enabled: false,
            wol_interface: String::new(),
            wol_targets: Vec::new(),
        }
    }
}

impl AtxConfig {
    pub fn normalize(&mut self) {
        if self.driver == AtxDriverType::None {
            self.enabled = false;
        }

        if self.driver != AtxDriverType::Gpio {
            self.led.enabled = false;
            self.hdd.enabled = false;
        }

        self.normalize_wol_targets();
    }

    /// Drop blank entries, canonicalize MAC notation and cap the list length.
    pub fn normalize_wol_targets(&mut self) {
        self.wol_targets.retain(|target| !target.mac.trim().is_empty());
        for target in &mut self.wol_targets {
            target.name = target.name.trim().to_string();
            if let Ok(mac) = crate::atx::normalize_mac_address(&target.mac) {
                target.mac = mac;
            } else {
                target.mac = target.mac.trim().to_uppercase();
            }
        }
        self.wol_targets.truncate(WOL_TARGET_MAX_COUNT);
    }

    /// Whether the console should offer any power-related control at all.
    ///
    /// ATX and WOL are independent: either one being enabled keeps the console
    /// power button visible.
    pub fn power_controls_available(&self) -> bool {
        self.enabled || self.wol_enabled
    }

    pub fn to_controller_config(&self) -> crate::atx::AtxControllerConfig {
        crate::atx::AtxControllerConfig {
            enabled: self.enabled,
            driver: self.driver,
            device: self.device.clone(),
            baud_rate: self.baud_rate,
            power: self.power.clone(),
            reset: self.reset.clone(),
            led: self.led.clone(),
            hdd: self.hdd.clone(),
        }
    }
}
