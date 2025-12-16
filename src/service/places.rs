use crate::file::File;
use crate::file::local::LocalFile;
use directories::UserDirs;

#[derive(Debug, Clone)]
pub struct Place {
    pub name: String,
    pub icon: String,
    pub file: String, // URI string to avoid trait object complexity
}

pub struct PlacesService {
    user_dirs: Option<UserDirs>,
}

impl PlacesService {
    pub fn new() -> Self {
        Self {
            user_dirs: UserDirs::new(),
        }
    }

    pub fn get_common_places(&self) -> Vec<Place> {
        let mut places = Vec::new();

        if let Some(ref dirs) = self.user_dirs {
            // Home
            places.push(Place {
                name: "Home".to_string(),
                icon: "user-home".to_string(),
                file: format!("file://{}", dirs.home_dir().to_string_lossy()),
            });

            // Desktop
            if let Some(desktop) = dirs.desktop_dir() {
                places.push(Place {
                    name: "Desktop".to_string(),
                    icon: "user-desktop".to_string(),
                    file: format!("file://{}", desktop.to_string_lossy()),
                });
            }

            // Documents
            if let Some(documents) = dirs.document_dir() {
                places.push(Place {
                    name: "Documents".to_string(),
                    icon: "folder-documents".to_string(),
                    file: format!("file://{}", documents.to_string_lossy()),
                });
            }

            // Downloads
            if let Some(downloads) = dirs.download_dir() {
                places.push(Place {
                    name: "Downloads".to_string(),
                    icon: "folder-download".to_string(),
                    file: format!("file://{}", downloads.to_string_lossy()),
                });
            }

            // Music
            if let Some(music) = dirs.audio_dir() {
                places.push(Place {
                    name: "Music".to_string(),
                    icon: "folder-music".to_string(),
                    file: format!("file://{}", music.to_string_lossy()),
                });
            }

            // Pictures
            if let Some(pictures) = dirs.picture_dir() {
                places.push(Place {
                    name: "Pictures".to_string(),
                    icon: "folder-pictures".to_string(),
                    file: format!("file://{}", pictures.to_string_lossy()),
                });
            }

            // Videos
            if let Some(videos) = dirs.video_dir() {
                places.push(Place {
                    name: "Videos".to_string(),
                    icon: "folder-videos".to_string(),
                    file: format!("file://{}", videos.to_string_lossy()),
                });
            }

            // Public
            if let Some(public) = dirs.public_dir() {
                places.push(Place {
                    name: "Public".to_string(),
                    icon: "folder-publicshare".to_string(),
                    file: format!("file://{}", public.to_string_lossy()),
                });
            }

            // Templates
            if let Some(templates) = dirs.template_dir() {
                places.push(Place {
                    name: "Templates".to_string(),
                    icon: "folder-templates".to_string(),
                    file: format!("file://{}", templates.to_string_lossy()),
                });
            }
        }

        places
    }
}

impl Default for PlacesService {
    fn default() -> Self {
        Self::new()
    }
}
