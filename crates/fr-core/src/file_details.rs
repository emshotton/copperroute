use crate::job::{FileFormat, java_path};
use fr_router::score::BoardStatistics;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Default)]
pub struct BoardFileDetails {
        pub size: u64,
            pub crc32: u32,
        pub format: FileFormat,
                                pub statistics: BoardStatistics,
            pub filename: String,
        pub directory_path: String,
        data_bytes: Vec<u8>,
}

impl BoardFileDetails {
                                        pub fn from_file(file: &Path) -> BoardFileDetails {
        let mut details = BoardFileDetails::default();
        let absolute = java_path::to_absolute_path(&file.to_string_lossy());
        details.set_filename(Some(&absolute)); 
        if let Ok(data) = std::fs::read(file) {
            let format = FileFormat::sniff_bytes(&data);
            details.set_data(data, format); 
        }
        details
    }

                pub fn from_board(board: &mut fr_board::Board) -> BoardFileDetails {
        BoardFileDetails {
            statistics: BoardStatistics::new(board), 
            ..BoardFileDetails::default()
        }
    }

                                                    pub fn calculate_crc32(data: &[u8]) -> u32 {
        let mut crc: u32 = 0xFFFF_FFFF;
        for &byte in data {
            crc ^= byte as u32;
            for _ in 0..8 {
                let mask = (crc & 1).wrapping_neg();
                crc = (crc >> 1) ^ (0xEDB8_8320 & mask);
            }
        }
        !crc
    }

                    pub fn get_absolute_path(&self) -> String {
        java_path::join2(&self.directory_path, &self.filename)
    }

        pub fn get_data(&self) -> &[u8] {
        &self.data_bytes
    }

                                                                                                                                pub fn set_data(&mut self, data: Vec<u8>, format: FileFormat) {
        self.size = data.len() as u64; 
        self.crc32 = BoardFileDetails::calculate_crc32(&data); 
        self.format = format;
        self.data_bytes = data; 
    }

        pub fn get_file(&self) -> Option<PathBuf> {
        if self.filename.is_empty() {
            return None;
        }
        Some(PathBuf::from(java_path::join2(
            &self.directory_path,
            &self.filename,
        )))
    }

        pub fn get_directory_path(&self) -> &str {
        &self.directory_path
    }

        pub fn get_filename(&self) -> &str {
        &self.filename
    }

                                                                                                                            pub fn set_filename(&mut self, filename: Option<&str>) {
        let Some(filename) = filename else {
            self.directory_path = String::new();
            self.filename = String::new();
            return;
        };

        let path = java_path::to_absolute_path(filename); 

        if filename.contains(crate::job::FILE_SEPARATOR) {
            self.directory_path = java_path::parent_of_normalized(&path).unwrap_or_default();
            self.directory_path = self.directory_path.replace("\\.\\", "\\");
            self.directory_path = self
                .directory_path
                .trim_end_matches(['/', '\\'])
                .to_string();
            self.directory_path = strip_backslash_and_any_char(&self.directory_path);
        } else {
            self.directory_path = String::new(); 
        }

        self.filename = java_path::file_name_of_normalized(&path).unwrap_or_default();

        if self.format == FileFormat::Unknown {
            self.format = FileFormat::from_path(Path::new(&self.filename));
        }

        if self.format != FileFormat::Unknown && !self.filename.contains('.') {
            let extension = self.format.default_extension();
            if !extension.is_empty() {
                self.filename = format!("{}.{extension}", self.filename);
            }
        }

    }

            pub fn get_filename_without_extension(&self) -> String {
        match self.filename.rfind('.') {
            Some(i) => self.filename[..i].to_string(),
            None => self.filename.clone(),
        }
    }
}

fn strip_backslash_and_any_char(s: &str) -> String {
    let chars: Vec<char> = s.chars().collect();
    if chars.len() >= 2 && chars[chars.len() - 2] == '\\' {
        return chars[..chars.len() - 2].iter().collect();
    }
    s.to_string()
}

