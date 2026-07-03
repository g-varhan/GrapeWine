use std::ffi::CString;
use std::os::raw::c_char;
use std::path::Path;

#[repr(C)]
pub struct PeMetadataRaw {
    pub is_valid: bool,
    pub is_64bit: bool,
    pub product_name: [u8; 128],
    pub product_version: [u8; 64],
    pub company_name: [u8; 128],
}

unsafe extern "C" {
    fn parse_pe_metadata(filepath: *const c_char, out_metadata: *mut PeMetadataRaw) -> bool;
    fn extract_pe_icon(filepath: *const c_char, out_ico_path: *const c_char) -> bool;
}

#[derive(Debug, Clone)]
pub struct PeMetadata {
    pub is_64bit: bool,
    pub product_name: String,
    pub product_version: String,
    pub company_name: String,
}

fn bytes_to_string(bytes: &[u8]) -> String {
    let len = bytes.iter().position(|&x| x == 0).unwrap_or(bytes.len());
    String::from_utf8_lossy(&bytes[..len]).into_owned().trim().to_string()
}

pub fn get_metadata<P: AsRef<Path>>(path: P) -> Option<PeMetadata> {
    let path_str = path.as_ref().to_str()?;
    let c_path = CString::new(path_str).ok()?;
    
    let mut raw = PeMetadataRaw {
        is_valid: false,
        is_64bit: false,
        product_name: [0; 128],
        product_version: [0; 64],
        company_name: [0; 128],
    };
    
    unsafe {
        if parse_pe_metadata(c_path.as_ptr(), &mut raw) && raw.is_valid {
            Some(PeMetadata {
                is_64bit: raw.is_64bit,
                product_name: bytes_to_string(&raw.product_name),
                product_version: bytes_to_string(&raw.product_version),
                company_name: bytes_to_string(&raw.company_name),
            })
        } else {
            None
        }
    }
}

pub fn extract_icon<P1: AsRef<Path>, P2: AsRef<Path>>(exe_path: P1, out_ico_path: P2) -> bool {
    let exe_str = match exe_path.as_ref().to_str() {
        Some(s) => s,
        None => return false,
    };
    let out_str = match out_ico_path.as_ref().to_str() {
        Some(s) => s,
        None => return false,
    };
    
    let c_exe = match CString::new(exe_str) {
        Ok(s) => s,
        Err(_) => return false,
    };
    let c_out = match CString::new(out_str) {
        Ok(s) => s,
        Err(_) => return false,
    };
    
    unsafe {
        extract_pe_icon(c_exe.as_ptr(), c_out.as_ptr())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bindings_compilation() {
        let meta = get_metadata("non_existent_file.exe");
        assert!(meta.is_none());
        
        let icon_extracted = extract_icon("non_existent_file.exe", "temp.ico");
        assert!(!icon_extracted);
    }
}
