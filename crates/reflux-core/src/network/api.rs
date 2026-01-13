use crate::error::Result;
use crate::network::HttpClient;
use std::collections::HashMap;

pub struct RefluxApi {
    client: HttpClient,
}

impl RefluxApi {
    pub fn new(server_address: String, api_key: String) -> Self {
        Self {
            client: HttpClient::new(server_address, api_key),
        }
    }

    pub async fn report_play(&self, form: HashMap<String, String>) -> Result<String> {
        self.client.post_form("api/songplayed", form).await
    }

    pub async fn report_unlock(
        &self,
        song_id: &str,
        difficulty: u8,
        unlock_type: u8,
    ) -> Result<String> {
        let mut form = HashMap::new();
        form.insert("songid".to_string(), song_id.to_string());
        form.insert("difficulty".to_string(), difficulty.to_string());
        form.insert("unlocktype".to_string(), unlock_type.to_string());

        self.client.post_form("api/unlocksong", form).await
    }

    pub async fn update_song(&self, form: HashMap<String, String>) -> Result<String> {
        self.client.post_form("api/updatesong", form).await
    }

    pub async fn upload_song_info(&self, form: HashMap<String, String>) -> Result<String> {
        self.client.post_form("api/uploadsonginfo", form).await
    }
}
