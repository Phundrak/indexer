pub enum CallType {
    Get,
    Head,
    Post,
    Put,
    Delete,
    Connect,
    Options,
    Trace,
    Patch
}

pub struct Appwrite {
    pub api_key: String,
    pub endpoint: String,
    pub project: String,
}

impl Appwrite {
    pub fn new(api_key: String, endpoint: String, project: String) -> Self {
        Self {
            api_key,
            endpoint,
            project
        }
    }
}

pub struct Storage {
    client: Appwrite
}

impl Storage {
    pub fn new(client: Appwrite) -> Self {
        Self {
            client
        }
    }

    pub fn list_buckets() -> String {
        let path = "/storage/buckets";

    }
}
