//! mDNS/DNS-SD announcement and browsing via `mdns-sd`.

use std::collections::HashMap;
use std::net::IpAddr;
use std::time::{Duration, Instant};

use mdns_sd::{ServiceDaemon, ServiceEvent, ServiceInfo};
use tauri::{AppHandle, Emitter, Manager};

use crate::error::Result;
use crate::protocol::{PROTO_VERSION, SERVICE_TYPE};
use crate::state::{AppState, DiscoveredDevice};

/// Starts or stops mDNS according to the "Automatic discovery" setting.
pub fn set_enabled(app: &AppHandle, enabled: bool) {
    let state = app.state::<AppState>();
    let mut slot = state.discovery.lock().unwrap();
    if enabled {
        if slot.is_none() {
            let port = state.listen_port.load(std::sync::atomic::Ordering::Relaxed);
            match start(app, port) {
                Ok(daemon) => *slot = Some(daemon),
                Err(e) => log::error!("mDNS discovery failed to start: {e}"),
            }
        }
        return;
    }
    if let Some(daemon) = slot.take() {
        let _ = daemon.shutdown();
        state.discovered.lock().unwrap().clear();
        drop(slot);
        emit_devices(app);
    }
}

fn start(app: &AppHandle, port: u16) -> Result<ServiceDaemon> {
    let daemon = ServiceDaemon::new()?;
    daemon.register(service_info(app, port)?)?;

    let rx = daemon.browse(SERVICE_TYPE)?;
    let browse_app = app.clone();
    tauri::async_runtime::spawn(async move {
        while let Ok(event) = rx.recv_async().await {
            handle_event(&browse_app, event);
        }
        log::debug!("mDNS browse stopped");
    });
    log::info!("mDNS discovery on");
    Ok(daemon)
}

/// Periodic refresh so stale discovered devices drop off the UI even without
/// a Removed event. Idle while discovery is off.
pub fn start_stale_sweep(app: &AppHandle) {
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        loop {
            tokio::time::sleep(Duration::from_secs(30)).await;
            let running = app.state::<AppState>().discovery.lock().unwrap().is_some();
            if running {
                emit_devices(&app);
            }
        }
    });
}

fn service_info(app: &AppHandle, port: u16) -> Result<ServiceInfo> {
    let state = app.state::<AppState>();
    let id = state.identity.device_id.clone();
    let name = state.settings.lock().unwrap().device_name.clone();
    let host = format!("nearclip-{}.local.", &id[..8]);

    let mut props: HashMap<String, String> = HashMap::new();
    props.insert("id".into(), id.clone());
    props.insert("name".into(), name);
    props.insert("v".into(), PROTO_VERSION.to_string());
    props.insert("pk".into(), state.identity.public_key_b64());

    Ok(ServiceInfo::new(SERVICE_TYPE, &id, &host, "", port, props)?.enable_addr_auto())
}

fn fullname(device_id: &str) -> String {
    format!("{device_id}.{SERVICE_TYPE}")
}

/// Re-registers the service after a device rename.
pub fn re_announce(app: &AppHandle) {
    let state = app.state::<AppState>();
    let port = state.listen_port.load(std::sync::atomic::Ordering::Relaxed);
    let guard = state.discovery.lock().unwrap();
    let Some(daemon) = guard.as_ref() else { return };
    let _ = daemon.unregister(&fullname(&state.identity.device_id));
    match service_info(app, port) {
        Ok(info) => {
            if let Err(e) = daemon.register(info) {
                log::error!("mDNS re-register failed: {e}");
            }
        }
        Err(e) => log::error!("mDNS service info failed: {e}"),
    }
}

fn handle_event(app: &AppHandle, event: ServiceEvent) {
    let state = app.state::<AppState>();
    match event {
        ServiceEvent::ServiceResolved(rs) => {
            let id = rs
                .txt_properties
                .get_property_val_str("id")
                .map(str::to_string)
                .or_else(|| rs.fullname.split('.').next().map(str::to_string));
            let Some(id) = id else { return };
            if id == state.identity.device_id {
                return;
            }
            let name = rs
                .txt_properties
                .get_property_val_str("name")
                .filter(|n| !n.is_empty())
                .unwrap_or("Unknown device")
                .to_string();
            let addrs: Vec<IpAddr> = rs
                .addresses
                .iter()
                .map(|a| a.to_ip_addr())
                .filter(|a| a.is_ipv4())
                .collect();
            if addrs.is_empty() {
                log::debug!("resolved {id} without IPv4 address, ignoring");
                return;
            }
            log::info!("discovered {name} ({id}) at {:?}:{}", addrs, rs.port);
            state.discovered.lock().unwrap().insert(
                id,
                DiscoveredDevice {
                    name,
                    addrs,
                    port: rs.port,
                    last_seen: Instant::now(),
                },
            );
            emit_devices(app);
        }
        ServiceEvent::ServiceRemoved(_, fullname) => {
            if let Some(id) = fullname.split('.').next() {
                if state.discovered.lock().unwrap().remove(id).is_some() {
                    log::info!("device {id} left the network");
                    emit_devices(app);
                }
            }
        }
        _ => {}
    }
}

pub fn emit_devices(app: &AppHandle) {
    let views = app.state::<AppState>().device_views();
    if let Err(e) = app.emit("devices-changed", views) {
        log::warn!("emit devices-changed failed: {e}");
    }
}
