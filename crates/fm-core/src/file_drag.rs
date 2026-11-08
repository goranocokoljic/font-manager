//! What a file drag out of the window carries (E44.4, ADR 0022): the font
//! files of a selection where the user keeps them (rule 2: never a copy),
//! each once, in the selection's order. A collection file holds several
//! faces and is dragged once; a file that is gone is left out and counted.

use std::path::PathBuf;

use fm_api::library::DragFiles;

/// The drag's files from the faces' files in selection order: each path
/// once, the ones not on disk (or with a name that is not UTF-8, which the
/// API cannot carry) left out and counted in `missing`.
pub fn drag_files(face_files: impl IntoIterator<Item = PathBuf>) -> DragFiles {
    let mut paths: Vec<String> = Vec::new();
    let mut missing: Vec<PathBuf> = Vec::new();
    for file in face_files {
        if missing.contains(&file) {
            continue;
        }
        match file.to_str() {
            Some(text) if file.is_file() => {
                if !paths.iter().any(|p| p == text) {
                    paths.push(text.to_owned());
                }
            }
            _ => missing.push(file),
        }
    }
    DragFiles {
        paths,
        missing: u32::try_from(missing.len()).unwrap_or(u32::MAX),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Scratch(PathBuf);

    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn each_file_once_in_order_and_gone_files_counted_once() {
        let dir =
            Scratch(std::env::temp_dir().join(format!("fm-file-drag-{}", std::process::id())));
        std::fs::create_dir_all(&dir.0).expect("scratch");
        let a = dir.0.join("A b.ttc");
        let b = dir.0.join("Ü.otf");
        std::fs::write(&a, b"ttcf").expect("a");
        std::fs::write(&b, b"OTTO").expect("b");
        let gone = dir.0.join("gone.ttf");
        let files = drag_files([
            b.clone(),
            a.clone(),
            b.clone(),
            gone.clone(),
            gone,
            dir.0.clone(),
        ]);
        assert_eq!(
            files.paths,
            [b.to_str().expect("utf-8"), a.to_str().expect("utf-8")]
        );
        assert_eq!(files.missing, 2, "the gone file once, and the folder");
    }

    #[test]
    fn an_empty_selection_drags_nothing() {
        let files = drag_files([]);
        assert!(files.paths.is_empty());
        assert_eq!(files.missing, 0);
    }
}
