use std::sync::{Arc, Mutex};
use zbus::interface;

pub struct CustomStatusNotifierWatcher {
    pub items: Arc<Mutex<Vec<String>>>,
}

#[interface(name = "org.kde.StatusNotifierWatcher")]
impl CustomStatusNotifierWatcher {
    async fn register_status_notifier_item(
        &mut self,
        service: &str,
        #[zbus(header)] header: zbus::message::Header<'_>,
        #[zbus(signal_context)] ctx: zbus::SignalContext<'_>,
    ) -> zbus::fdo::Result<()> {
        let sender = header.sender().map(|s| s.to_string()).unwrap_or_else(|| service.to_string());
        crate::backend::log_to_file(&format!("Tray Watcher: register_status_notifier_item service='{}', sender='{}'", service, sender));
        {
            let mut items = self.items.lock().unwrap();
            let entry = if service.starts_with('/') {
                format!("{}|{}", sender, service)
            } else {
                format!("{}|/StatusNotifierItem", service)
            };
            
            if !items.contains(&entry) {
                crate::backend::log_to_file(&format!("Tray Watcher: Adding item '{}'", entry));
                items.push(entry);
                crate::backend::log_to_file(&format!("Tray Watcher: Current watcher items: {:?}", *items));
            }
        }
        
        let _ = Self::status_notifier_item_registered(&ctx, service).await;
        Ok(())
    }
    
    async fn register_status_notifier_host(
        &mut self, 
        service: &str,
        #[zbus(signal_context)] ctx: zbus::SignalContext<'_>,
    ) -> zbus::fdo::Result<()> {
        crate::backend::log_to_file(&format!("Tray Watcher: register_status_notifier_host service='{}'", service));
        let _ = Self::status_notifier_host_registered(&ctx).await;
        Ok(())
    }

    #[zbus(signal)]
    pub async fn status_notifier_item_registered(
        ctx: &zbus::SignalContext<'_>,
        service: &str,
    ) -> zbus::Result<()>;

    #[zbus(signal)]
    pub async fn status_notifier_host_registered(
        ctx: &zbus::SignalContext<'_>,
    ) -> zbus::Result<()>;

    #[zbus(property)]
    async fn registered_status_notifier_items(&self) -> Vec<String> {
        let raw_items = self.items.lock().unwrap().clone();
        let items: Vec<String> = raw_items.iter().map(|s| s.replace('|', "")).collect();
        crate::backend::log_to_file(&format!("Tray Watcher: registered_status_notifier_items queried. raw={:?}, returning={:?}", raw_items, items));
        items
    }

    #[zbus(property)]
    async fn is_status_notifier_host_registered(&self) -> bool {
        true
    }

    #[zbus(property)]
    async fn protocol_version(&self) -> i32 {
        1
    }
}

pub async fn emit_host_registered(ctx: &zbus::SignalContext<'_>) -> zbus::Result<()> {
    CustomStatusNotifierWatcher::status_notifier_host_registered(ctx).await
}
