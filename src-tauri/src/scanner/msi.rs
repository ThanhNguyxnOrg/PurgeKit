use serde::{Deserialize, Serialize};
use windows_sys::Win32::System::ApplicationInstallationAndServicing::{
    MsiEnumProductsW, MsiGetProductInfoW
};
use windows_sys::Win32::Foundation::{ERROR_SUCCESS, ERROR_NO_MORE_ITEMS};
use crate::scanner::RemnantItem;
use std::fs;
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

        // Convert UTF-16 GUID to String, dropping the trailing null char dynamically
        let guid_len = product_code.iter().position(|&c| c == 0).unwrap_or(38);
        let guid = String::from_utf16_lossy(&product_code[..guid_len]);

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
    let app_token_lower = app_token.to_lowercase().trim().to_string();
    if app_token_lower.len() < 3 {
        return remnants;
    }

    // Safety: Blacklist core Microsoft and generic system terms to prevent catastrophic false positives
    let blacklist = [
        "microsoft", "windows", "visual", "redistributable", "update", "installer",
        "service", "runtime", "package", "driver", "framework", "system", "tools"
    ];
    if blacklist.contains(&app_token_lower.as_str()) {
        return remnants;
    }

    // 1. Enumerate all active MSI products and collect their LocalPackages
    // ANY file in this set belongs to a currently installed, healthy product and MUST NOT be deleted!
    let active_products = enumerate_msi_products();
    let active_local_packages: std::collections::HashSet<String> = active_products
        .iter()
        .filter_map(|p| p.local_pkg.as_ref())
        .map(|pkg| crate::winutil::canonicalize_path_safety(pkg).to_string_lossy().to_lowercase())
        .collect();

    // 2. Scan C:\Windows\Installer for orphaned .msi and .msp files
    let windir = std::env::var("SystemRoot").unwrap_or_else(|_| "C:\\Windows".to_string());
    let installer_dir = Path::new(&windir).join("Installer");
    if !installer_dir.exists() || !installer_dir.is_dir() {
        return remnants;
    }

    let entries = match fs::read_dir(&installer_dir) {
        Ok(e) => e,
        Err(_) => return remnants,
    };

    for entry in entries.filter_map(|e| e.ok()) {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }

        let ext = path.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default();
        if ext != "msi" && ext != "msp" {
            continue;
        }

        let path_canon = crate::winutil::canonicalize_path_safety(&path.to_string_lossy());
        let path_canon_lower = path_canon.to_string_lossy().to_lowercase();

        // If this package is registered to an ACTIVE product, NEVER delete it!
        if active_local_packages.contains(&path_canon_lower) {
            continue;
        }

        // Check if filename contains the token
        let file_name = path.file_name().map(|n| n.to_string_lossy().to_lowercase()).unwrap_or_default();
        if file_name.contains(&app_token_lower) {
            remnants.push(RemnantItem {
                path: path.to_string_lossy().to_string(),
                item_type: "File".to_string(),
                size: entry.metadata().map(|m| m.len()).unwrap_or(0),
                confidence: "Medium".to_string(),
                score: 65,
            });
        }
    }

    remnants
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_enumerate_msi_products_no_crash() {
        let products = enumerate_msi_products();
        println!("Found {} msi products", products.len());
    }

    #[test]
    fn test_scan_msi_remnants_no_crash() {
        let remnants = scan_msi_remnants("nonexistentappnamexyz", None);
        println!("Found {} msi remnants", remnants.len());
    }
}
