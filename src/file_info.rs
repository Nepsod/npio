use std::collections::HashMap;


#[derive(Debug, Clone, PartialEq)]
pub enum FileAttributeType {
    String(String),
    Stringv(Vec<String>),
    ByteString(Vec<u8>),
    Boolean(bool),
    Uint32(u32),
    Int32(i32),
    Uint64(u64),
    Int64(i64),
    Object(Box<FileAttributeType>), // Simplified object representation
    Invalid,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileType {
    Unknown,
    Regular,
    Directory,
    SymbolicLink,
    Special,
    Shortcut,
    Mountable,
}

#[derive(Debug, Clone)]
pub struct FileInfo {
    attributes: HashMap<String, FileAttributeType>,
}

impl FileInfo {
    pub fn new() -> Self {
        Self {
            attributes: HashMap::new(),
        }
    }

    pub fn set_attribute(&mut self, key: &str, value: FileAttributeType) {
        self.attributes.insert(key.to_string(), value);
    }

    pub fn get_attribute(&self, key: &str) -> Option<&FileAttributeType> {
        self.attributes.get(key)
    }

    pub fn has_attribute(&self, key: &str) -> bool {
        self.attributes.contains_key(key)
    }

    pub fn remove_attribute(&mut self, key: &str) {
        self.attributes.remove(key);
    }

    // Common attributes helpers
    
    pub fn set_name(&mut self, name: &str) {
        self.set_attribute("standard::name", FileAttributeType::String(name.to_string()));
    }

    pub fn get_name(&self) -> Option<&str> {
        match self.get_attribute("standard::name") {
            Some(FileAttributeType::String(s)) => Some(s),
            _ => None,
        }
    }

    pub fn set_display_name(&mut self, name: &str) {
        self.set_attribute("standard::display-name", FileAttributeType::String(name.to_string()));
    }

    pub fn get_display_name(&self) -> Option<&str> {
        match self.get_attribute("standard::display-name") {
            Some(FileAttributeType::String(s)) => Some(s),
            _ => None,
        }
    }

    pub fn set_file_type(&mut self, file_type: FileType) {
        let val = match file_type {
            FileType::Unknown => 0,
            FileType::Regular => 1,
            FileType::Directory => 2,
            FileType::SymbolicLink => 3,
            FileType::Special => 4,
            FileType::Shortcut => 5,
            FileType::Mountable => 6,
        };
        self.set_attribute("standard::type", FileAttributeType::Uint32(val));
    }

    pub fn get_file_type(&self) -> FileType {
        match self.get_attribute("standard::type") {
            Some(FileAttributeType::Uint32(val)) => match val {
                1 => FileType::Regular,
                2 => FileType::Directory,
                3 => FileType::SymbolicLink,
                4 => FileType::Special,
                5 => FileType::Shortcut,
                6 => FileType::Mountable,
                _ => FileType::Unknown,
            },
            _ => FileType::Unknown,
        }
    }

    pub fn set_size(&mut self, size: u64) {
        self.set_attribute("standard::size", FileAttributeType::Uint64(size));
    }

    pub fn get_size(&self) -> i64 {
        match self.get_attribute("standard::size") {
            Some(FileAttributeType::Uint64(s)) => *s as i64,
            Some(FileAttributeType::Int64(s)) => *s,
            _ => 0,
        }
    }
    
    pub fn set_content_type(&mut self, content_type: &str) {
        self.set_attribute("standard::content-type", FileAttributeType::String(content_type.to_string()));
    }
    
    pub fn get_content_type(&self) -> Option<&str> {
        match self.get_attribute("standard::content-type") {
            Some(FileAttributeType::String(s)) => Some(s),
            _ => None,
        }
    }

    pub fn set_modification_time(&mut self, time: u64) {
        self.set_attribute("time::modified", FileAttributeType::Uint64(time));
    }
}

impl Default for FileInfo {
    fn default() -> Self {
        Self::new()
    }
}
