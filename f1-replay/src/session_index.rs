use bevy::prelude::Resource;

#[derive(Debug, Clone)]
pub struct SessionEntry {
    pub path: String,
    pub display: String,
}

#[derive(Resource)]
pub struct SessionIndex {
    pub entries: Vec<SessionEntry>,
}

impl SessionIndex {
    pub fn scan() -> Self {
        let entries = vec![
            SessionEntry {
                path: "/2023/2023-07-30_Belgian_Grand_Prix/2023-07-30_Race/".into(),
                display: "2023 Belgian GP · Race".into(),
            },
            SessionEntry {
                path: "/2023/2023-07-09_British_Grand_Prix/2023-07-09_Race/".into(),
                display: "2023 British GP · Race".into(),
            },
            SessionEntry {
                path: "/2024/2024-03-02_Bahrain_Grand_Prix/2024-03-02_Race/".into(),
                display: "2024 Bahrain GP · Race".into(),
            },
        ];
        Self { entries }
    }
}
