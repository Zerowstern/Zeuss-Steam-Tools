// Simple Protobuf Parser for Steam Manifests
use std::fs;
use std::path::Path;

#[derive(Debug)]
pub struct ManifestSummary {
    pub total_size: u64,
    pub file_count: u64,
    pub top_files: Vec<String>,
    pub os: String, // "Windows", "MacOS", "Linux", "Unknown"
}

pub fn parse_manifest(path: &Path) -> Result<ManifestSummary, String> {
    let data = fs::read(path).map_err(|e| e.to_string())?;
    
    // Check for magic header 0x71F617D0 (sometimes used in custom formats)
    // But assuming standard protobuf for now.
    
    let mut cursor = 0;
    let mut total_size = 0;
    let mut file_count = 0;
    let mut filenames = Vec::new();

    // Field 1 is repeated FileMapping
    while cursor < data.len() {
        let (tag, wire) = match read_tag(&data, &mut cursor) {
            Ok(v) => v,
            Err(_) => break, // EOF or error
        };
        
        if tag == 1 && wire == 2 { // 'files' repeated message
             let len = read_varint(&data, &mut cursor).unwrap_or(0);
             let end = cursor + len as usize;
             if end > data.len() { break; }
             
             // Parse FileMapping
             let mut f_size = 0;
             let mut f_name = String::new();
             
             let mut inner_cursor = cursor;
             while inner_cursor < end {
                 let (f_tag, f_wire) = match read_tag(&data, &mut inner_cursor) {
                     Ok(v) => v,
                     Err(_) => break,
                 };
                 
                 if f_tag == 1 && f_wire == 2 { // filename
                     if let Ok(s) = read_string(&data, &mut inner_cursor) {
                         f_name = s;
                     }
                 } else if f_tag == 2 && f_wire == 0 { // size
                     if let Ok(s) = read_varint(&data, &mut inner_cursor) {
                         f_size = s;
                     }
                 } else {
                     skip_field(&data, &mut inner_cursor, f_wire).ok();
                 }
             }
             
             total_size += f_size;
             file_count += 1;
             if filenames.len() < 10 { // Keep first few for OS guessing if needed
                 filenames.push(f_name.clone());
             }
             // Reservoir sampling or check specific extensions for OS
             if f_name.ends_with(".exe") || f_name.ends_with(".dll") {
                 filenames.push(f_name); 
             } else if f_name.contains(".app/") || f_name.ends_with(".dmg") {
                 filenames.push(f_name);
             } else if f_name.ends_with(".so") || f_name.ends_with(".deb") {
                 filenames.push(f_name);
             }

             cursor = end;
        } else {
             skip_field(&data, &mut cursor, wire).ok();
        }
    }

    let os = guess_os(&filenames);
    
    Ok(ManifestSummary {
        total_size,
        file_count,
        top_files: filenames.into_iter().take(5).collect(),
        os,
    })
}

fn guess_os(filenames: &[String]) -> String {
    let mut win_score = 0;
    let mut mac_score = 0;
    let mut lin_score = 0;

    for name in filenames {
        let lower = name.to_lowercase();
        if lower.ends_with(".exe") || lower.ends_with(".dll") || lower.contains("windows") {
            win_score += 1;
        }
        if lower.contains(".app/") || lower.ends_with(".dmg") || lower.contains("macos") || lower.contains("osx") {
            mac_score += 1;
        }
        if lower.ends_with(".so") || lower.ends_with(".deb") || lower.contains("linux") {
            lin_score += 1;
        }
    }

    if win_score > mac_score && win_score > lin_score {
        "Windows".to_string()
    } else if mac_score > win_score && mac_score > lin_score {
        "MacOS".to_string()
    } else if lin_score > win_score && lin_score > mac_score {
        "Linux".to_string()
    } else {
        "Unknown".to_string()
    }
}

// Protobuf primitives
fn read_tag(data: &[u8], cursor: &mut usize) -> Result<(u32, u8), ()> {
    let val = read_varint(data, cursor)?;
    Ok(((val >> 3) as u32, (val & 0x07) as u8))
}

fn read_varint(data: &[u8], cursor: &mut usize) -> Result<u64, ()> {
    let mut result = 0u64;
    let mut shift = 0;
    loop {
        if *cursor >= data.len() { return Err(()); }
        let b = data[*cursor];
        *cursor += 1;
        result |= ((b & 0x7F) as u64) << shift;
        if (b & 0x80) == 0 {
            break;
        }
        shift += 7;
        if shift > 64 { return Err(()); }
    }
    Ok(result)
}

fn read_string(data: &[u8], cursor: &mut usize) -> Result<String, ()> {
    let len = read_varint(data, cursor)? as usize;
    if *cursor + len > data.len() { return Err(()); }
    let bytes = &data[*cursor..*cursor + len];
    *cursor += len;
    String::from_utf8(bytes.to_vec()).map_err(|_| ())
}

fn skip_field(data: &[u8], cursor: &mut usize, wire: u8) -> Result<(), ()> {
     match wire {
         0 => { read_varint(data, cursor)?; }, // Varint
         1 => { *cursor += 8; }, // 64bit
         2 => { // Length delimited
             let len = read_varint(data, cursor)? as usize;
             *cursor += len;
         },
         5 => { *cursor += 4; }, // 32bit
         _ => return Err(()), // StartGroup/EndGroup deprecated
     }
     Ok(())
}
