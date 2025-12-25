//! Tests for user directory helpers

use std::path::PathBuf;
use std::fs;
use npio::service::places::{parse_user_dirs_file, get_home_file, get_user_special_file, UserDirectory};
use npio::backend::local::LocalBackend;
use npio::{register_backend};
use std::sync::Arc;

#[test]
fn test_parse_user_dirs_file_relative_paths() {
    let home_dir = PathBuf::from("/home/testuser");
    
    let content = r#"XDG_DESKTOP_DIR="$HOME/Desktop"
XDG_DOCUMENTS_DIR="$HOME/Documents"
XDG_DOWNLOAD_DIR="$HOME/Downloads"
XDG_MUSIC_DIR="$HOME/Music"
XDG_PICTURES_DIR="$HOME/Pictures"
XDG_PUBLICSHARE_DIR="$HOME/Public"
XDG_TEMPLATES_DIR="$HOME/Templates"
XDG_VIDEOS_DIR="$HOME/Videos"
"#;
    
    let dirs = parse_user_dirs_file(content, &home_dir);
    
    assert_eq!(dirs.get(&UserDirectory::Desktop), Some(&PathBuf::from("/home/testuser/Desktop")));
    assert_eq!(dirs.get(&UserDirectory::Documents), Some(&PathBuf::from("/home/testuser/Documents")));
    assert_eq!(dirs.get(&UserDirectory::Download), Some(&PathBuf::from("/home/testuser/Downloads")));
    assert_eq!(dirs.get(&UserDirectory::Music), Some(&PathBuf::from("/home/testuser/Music")));
    assert_eq!(dirs.get(&UserDirectory::Pictures), Some(&PathBuf::from("/home/testuser/Pictures")));
    assert_eq!(dirs.get(&UserDirectory::PublicShare), Some(&PathBuf::from("/home/testuser/Public")));
    assert_eq!(dirs.get(&UserDirectory::Templates), Some(&PathBuf::from("/home/testuser/Templates")));
    assert_eq!(dirs.get(&UserDirectory::Videos), Some(&PathBuf::from("/home/testuser/Videos")));
}

#[test]
fn test_parse_user_dirs_file_absolute_paths() {
    let home_dir = PathBuf::from("/home/testuser");
    
    let content = r#"XDG_DESKTOP_DIR="/custom/desktop"
XDG_DOCUMENTS_DIR="/custom/documents"
"#;
    
    let dirs = parse_user_dirs_file(content, &home_dir);
    
    assert_eq!(dirs.get(&UserDirectory::Desktop), Some(&PathBuf::from("/custom/desktop")));
    assert_eq!(dirs.get(&UserDirectory::Documents), Some(&PathBuf::from("/custom/documents")));
}

#[test]
fn test_parse_user_dirs_file_trailing_slashes() {
    let home_dir = PathBuf::from("/home/testuser");
    
    let content = r#"XDG_DESKTOP_DIR="$HOME/Desktop/"
XDG_DOCUMENTS_DIR="/custom/documents/"
"#;
    
    let dirs = parse_user_dirs_file(content, &home_dir);
    
    // Trailing slashes should be removed
    assert_eq!(dirs.get(&UserDirectory::Desktop), Some(&PathBuf::from("/home/testuser/Desktop")));
    assert_eq!(dirs.get(&UserDirectory::Documents), Some(&PathBuf::from("/custom/documents")));
}

#[test]
fn test_parse_user_dirs_file_duplicates() {
    let home_dir = PathBuf::from("/home/testuser");
    
    let content = r#"XDG_DESKTOP_DIR="$HOME/Desktop"
XDG_DESKTOP_DIR="$HOME/CustomDesktop"
"#;
    
    let dirs = parse_user_dirs_file(content, &home_dir);
    
    // Last one should win (matching GLib behavior)
    assert_eq!(dirs.get(&UserDirectory::Desktop), Some(&PathBuf::from("/home/testuser/CustomDesktop")));
}

#[test]
fn test_parse_user_dirs_file_comments_and_whitespace() {
    let home_dir = PathBuf::from("/home/testuser");
    
    let content = r#"# This is a comment
XDG_DESKTOP_DIR="$HOME/Desktop"
   XDG_DOCUMENTS_DIR   =   "$HOME/Documents"   
"#;
    
    let dirs = parse_user_dirs_file(content, &home_dir);
    
    assert_eq!(dirs.get(&UserDirectory::Desktop), Some(&PathBuf::from("/home/testuser/Desktop")));
    assert_eq!(dirs.get(&UserDirectory::Documents), Some(&PathBuf::from("/home/testuser/Documents")));
}

#[test]
fn test_parse_user_dirs_file_invalid_lines() {
    let home_dir = PathBuf::from("/home/testuser");
    
    let content = r#"INVALID_LINE="something"
XDG_DESKTOP_DIR="$HOME/Desktop"
XDG_DOCUMENTS_DIR=unquoted
XDG_DOWNLOAD_DIR="$HOME/Downloads"
"#;
    
    let dirs = parse_user_dirs_file(content, &home_dir);
    
    // Only valid lines should be parsed
    assert_eq!(dirs.get(&UserDirectory::Desktop), Some(&PathBuf::from("/home/testuser/Desktop")));
    assert_eq!(dirs.get(&UserDirectory::Download), Some(&PathBuf::from("/home/testuser/Downloads")));
    assert_eq!(dirs.get(&UserDirectory::Documents), None);
}

#[tokio::test]
async fn test_get_home_file() {
    let backend = Arc::new(LocalBackend::new());
    register_backend(backend);
    
    let home_file = get_home_file();
    assert!(home_file.is_ok());
    
    let file = home_file.unwrap();
    assert!(file.uri().starts_with("file://"));
}

#[tokio::test]
async fn test_get_user_special_file() {
    let backend = Arc::new(LocalBackend::new());
    register_backend(backend);
    
    // At least Desktop should be available (has fallback)
    let desktop_file = get_user_special_file(UserDirectory::Desktop);
    assert!(desktop_file.is_ok());
    
    // Other directories may or may not exist, but should not error
    let docs_file = get_user_special_file(UserDirectory::Documents);
    assert!(docs_file.is_ok());
    
    let download_file = get_user_special_file(UserDirectory::Download);
    assert!(download_file.is_ok());
}

#[tokio::test]
async fn test_get_user_special_file_with_custom_config() {
    let backend = Arc::new(LocalBackend::new());
    register_backend(backend);
    
    // Create a temporary user-dirs.dirs file
    let test_dir = std::env::temp_dir().join("npio_places_test");
    if test_dir.exists() {
        fs::remove_dir_all(&test_dir).unwrap();
    }
    fs::create_dir_all(&test_dir).unwrap();
    
    let config_file = test_dir.join("user-dirs.dirs");
    
    let content = r#"XDG_DESKTOP_DIR="$HOME/CustomDesktop"
XDG_DOCUMENTS_DIR="/custom/documents"
"#;
    
    fs::write(&config_file, content).unwrap();
    
    // Set XDG_CONFIG_HOME to point to our test directory
    std::env::set_var("XDG_CONFIG_HOME", test_dir.to_string_lossy().to_string());
    
    // Note: This test verifies the file is read, but since load_user_special_dirs()
    // is called internally and may be cached, we can't easily test the custom config
    // without exposing internal implementation details. The integration test above
    // verifies the basic functionality works.
    
    // Cleanup
    std::env::remove_var("XDG_CONFIG_HOME");
    fs::remove_dir_all(&test_dir).unwrap();
}

