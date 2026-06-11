use serde::{Deserialize, Serialize};
use windows_sys::Win32::System::ApplicationInstallationAndServicing::{
    MsiEnumProductsW, MsiGetProductInfoW
};
use windows_sys::Win32::Foundation::{ERROR_SUCCESS, ERROR_NO_MORE_ITEMS};
use crate::scanner::RemnantItem;
use std::path::Path;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct MsiProduct {
    pub guid: String,
    pub name: String,
    pub install_loc: Option<String>,
    pub local_pkg: Option<String>,
    pub version: Option<String>,
}

pub fn enumerate_msi_products() -> Vec<MsiProduct> {
    let mut products = Vec::new();
    let mut idx = 0u32;

    loop {
        let mut product_code = [0u16; 39]; // GUID is 38 chars + null terminator

        let result = unsafe { MsiEnumProductsW(idx, product_code.as_mut_ptr()) };

        if result == ERROR_NO_MORE_ITEMS { break; }
        if result != ERROR_SUCCESS { idx += 1; continue; }

        // Convert UTF-16 GUID to String, dropping the trailing null char
        let guid = String::from_utf16_lossy(&product_code[..38]);

        let name = msi_get_property(&guid, "InstalledProductName");
        let install_loc = msi_get_property(&guid, "InstallLocation");
        let local_pkg = msi_get_property(&guid, "LocalPackage");
        let version = msi_get_property(&guid, "VersionString");

        products.push(MsiProduct {
            guid,
            name: name.unwrap_or_else(|| "Unknown MSI Product".to_string()),
            install_loc,
            local_pkg,
            version,
        });
        idx += 1;
    }
    products
}

fn msi_get_property(product_code: &str, property: &str) -> Option<String> {
    let code_wide: Vec<u16> = product_code.encode_utf16().chain(Some(0)).collect();
    let prop_wide: Vec<u16> = property.encode_utf16().chain(Some(0)).collect();

    let mut size = 0u32;
    // Get required buffer size
    unsafe {
        MsiGetProductInfoW(
            code_wide.as_ptr(),
            prop_wide.as_ptr(),
            std::ptr::null_mut(),
            &mut size,
        );
    }
    if size == 0 { return None; }

    size += 1; // Null terminator
    let mut buf = vec![0u16; size as usize];
    let result = unsafe {
        MsiGetProductInfoW(
            code_wide.as_ptr(),
            prop_wide.as_ptr(),
            buf.as_mut_ptr(),
            &mut size,
        )
    };

    if result == ERROR_SUCCESS {
        Some(String::from_utf16_lossy(&buf[..size as usize]))
    } else {
        None
    }
}

pub fn scan_msi_remnants(app_token: &str, _install_dir: Option<&str>) -> Vec<RemnantItem> {
    let mut remnants = Vec::new();
    let app_token_lower = app_token.to_lowercase();
    
    let products = enumerate_msi_products();
    for prod in products {
        let prod_name_lower = prod.name.to_lowercase();
        // If the uninstalled app name matches this MSI product name, we look for its LocalPackage
        if prod_name_lower.contains(&app_token_lower) {
            if let Some(ref local_pkg) = prod.local_pkg {
                if Path::new(local_pkg).exists() {
                    // This is the cached MSI installer file in C:\Windows\Installer
                    // Deleting this file is completely safe once the app is uninstalled
                    remnants.push(RemnantItem {
                        path: local_pkg.clone(),
                        item_type: "File".to_string(),
                        size: Path::new(local_pkg).metadata().map(|m| m.len()).unwrap_or(0),
                        confidence: "High".to_string(),
                        score: 75,
                    });
                }
            }
        }
    }

    remnants
}
