use anyhow::{Ok, Result};
use const_format::formatcp;
use elementtree::Element;
use rayon::prelude::*;
use std::collections::HashSet;
use std::fs::{self};
use std::sync::Mutex;
use std::time::Instant;
use walkdir::WalkDir;

const ROOT_PATH: &str = r"E:\Projects\Git\Working\MoneyTrackerPro";

const SOURCE_CODE_FOLDERS: [&str; 3] = [
    formatcp!(r"{}\MoneyTracker.Universal\", ROOT_PATH),
    formatcp!(r"{}\MoneyTracker.Universal.Core\", ROOT_PATH),
    formatcp!(r"{}\MoneyTracker.Universal.Maui\", ROOT_PATH),
];

const SOURCE_CODE_FILE_TYPES: [&str; 2] = ["cs", "xaml"];

const LINES_FOLDER: &str = formatcp!(r"{}\MoneyTracker.Universal.Core\Strings", ROOT_PATH);
const REFERENCE_FILE: &str = formatcp!(r"{}\en-US\Resources.resw", LINES_FOLDER);

fn read_xml_file(file: &str) -> Result<Element> {
    let mut xml = std::fs::read_to_string(file)?;
    if xml.starts_with('\u{feff}') {
        xml.remove(0); // remove BOM char if present
    }

    let root = Element::from_reader(xml.as_bytes())?;
    Ok(root)
}

// Extract identifiers that follow a dot: ".Identifier" -> "Identifier"
fn extract_keys_from_text_to_set(text: &str) -> HashSet<String> {
    let mut out = HashSet::<String>::with_capacity(256);

    let mut start = 0;
    while let Some(dot_index) = text[start..].find('.') {
        let dot_index = start + dot_index;
        let end_index = text[dot_index + 1..]
            .find(|c: char| !c.is_alphanumeric() && c != '_')
            .map_or(text.len(), |i| dot_index + 1 + i);
        let key = &text[dot_index + 1..end_index];
        out.insert(key.to_string());
        start = end_index;
    }

    out
}

fn find_unused_translations(reference_lines: &[&str]) {
    let paths: Vec<String> = SOURCE_CODE_FOLDERS
        .iter()
        .flat_map(|folder| {
            WalkDir::new(folder).into_iter().filter_map(|e| e.ok()).filter(|e| {
                e.file_type().is_file()
                    && e.file_name() != "Strings.cs"
                    && e.path()
                        .extension()
                        .and_then(|ext| ext.to_str())
                        .is_some_and(|ext| SOURCE_CODE_FILE_TYPES.contains(&ext))
            })
        })
        .map(|f| f.path().to_str().unwrap().to_string())
        .collect();

    let mut used_keys = HashSet::<String>::with_capacity(1024);
    let used_keys_mutex = Mutex::new(&mut used_keys);

    paths.par_iter().for_each(|path| {
        let text = fs::read_to_string(path).unwrap_or_else(|_| panic!("Failed to read file: {:?}", path));
        if text.is_empty() {
            return;
        }

        let local = extract_keys_from_text_to_set(&text);

        if !local.is_empty() {
            used_keys_mutex.lock().unwrap().extend(local);
        }
    });

    for &key in reference_lines {
        if !used_keys.contains(key) {
            println!("Unused line: {key}");
        }
    }
}

fn find_missed_translations(reference_lines: &[&str]) {
    let files: Vec<String> = WalkDir::new(LINES_FOLDER)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.file_type().is_file()
                && e.path() != std::path::Path::new(REFERENCE_FILE)
                && e.path().extension().and_then(|e| e.to_str()) == Some("resw")
        })
        .map(|e| e.path().to_str().unwrap().to_string())
        .collect();

    files.par_iter().for_each(|file| {
        let root = read_xml_file(file).unwrap();

        let mut sb = String::from(format!("Name: {file}").as_str());

        let mut file_keys = HashSet::<String>::with_capacity(1024);
        let mut untranslated_count = 0;

        for data in root.find_all("data") {
            let name = data.get_attr("name").unwrap().to_string();
            file_keys.insert(name);

            if data.find("comment").is_some_and(|c| c.text().starts_with("Check")) {
                untranslated_count += 1;
            }
        }

        // missed (present in reference but missing in file)
        for ref_line in reference_lines {
            if !file_keys.contains(*ref_line) && *ref_line != "NewsContent" {
                sb.push_str(format!("\n  Missed line: {ref_line}").as_str());
            }
        }

        // extra (present in file but not in reference)
        for key in file_keys {
            if !reference_lines.contains(&key.as_str()) {
                sb.push_str(format!("\n  Extra line: {key}").as_str());
            }
        }

        sb.push_str(format!("\n  Untranslated lines: {untranslated_count}").as_str());

        println!("{sb}");
    });
}

fn main() -> Result<()> {
    let sw = Instant::now();

    let root = read_xml_file(REFERENCE_FILE)?;

    let reference_lines: Vec<&str> = root.find_all("data").map(|e| e.get_attr("name").unwrap()).collect();

    find_unused_translations(&reference_lines);

    println!();

    find_missed_translations(&reference_lines);

    println!("Time: {:.2?}", sw.elapsed());

    Ok(())
}
