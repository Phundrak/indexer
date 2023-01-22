use color_eyre::eyre::Result;
use rocket::http::Status;
use rocket::request::{FromRequest, Outcome, Request};
use rocket::serde::Deserialize;
use tracing::info;

use super::ServerState;

#[derive(Default, Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
#[serde(crate = "rocket::serde")]
struct UserSessions {
    total: i64,
    sessions: Vec<Sessions>,
}

#[derive(Default, Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
#[serde(crate = "rocket::serde")]
struct Sessions {
    #[serde(rename = "$id")]
    id: String,
    #[serde(rename = "$createdAt")]
    created_at: String,
    user_id: String,
    expire: String,
    provider: String,
    provider_uid: String,
    provider_access_token: String,
    provider_access_token_expiry: String,
    provider_refresh_token: String,
    ip: String,
    os_code: String,
    os_name: String,
    os_version: String,
    client_type: String,
    client_code: String,
    client_name: String,
    client_version: String,
    client_engine: String,
    client_engine_version: String,
    device_name: String,
    device_brand: String,
    device_model: String,
    country_code: String,
    country_name: String,
    current: bool,
}

/// HTTP header for a user's session
///
/// This header `X-User-Auth` must be a single string in the form
/// `userId;userSessionId` with `userId` and `userSessionId` being
/// variables given by Appwrite to users that are logged in.
#[derive(Debug, Copy, Clone)]
pub struct UserSession<'r>(&'r str);

#[derive(Debug)]
pub enum UserSessionError {
    Missing,
    Invalid,
}

#[rocket::async_trait]
impl<'r> FromRequest<'r> for UserSession<'r> {
    type Error = UserSessionError;

    async fn from_request(
        request: &'r Request<'_>,
    ) -> Outcome<Self, Self::Error> {
        /// Retrieve all sesssions from user with `userId`. If
        /// `userSessionId` is among them, then the user is connected
        /// and return true.
        async fn is_valid(
            user_id: &str,
            user_session_id: &str,
            state: &ServerState,
        ) -> Result<bool> {
            let client = reqwest::Client::new();
            let url =
                format!("{}/users/{user_id}/sessions", state.appwrite_endpoint);
            let response = client
                .get(url.clone())
                .header("X-Appwrite-Key", state.appwrite_key.clone())
                .header("X-Appwrite-Project", state.appwrite_project.clone())
                .header("Content-Type", "application/json")
                .send()
                .await?
                .json::<UserSessions>()
                .await?;
            Ok(response
                .sessions
                .iter()
                .any(|session| session.id == user_session_id))
        }

        let server_state = request.rocket().state::<ServerState>().unwrap();
        match request.headers().get_one("x-user-auth") {
            None => Outcome::Failure((
                Status::BadRequest,
                UserSessionError::Missing,
            )),
            Some(key) => {
                let key: Vec<_> = key.split(';').collect();
                if key.len() != 2 {
                    return Outcome::Failure((
                        Status::BadRequest,
                        UserSessionError::Invalid,
                    ));
                }
                let user_id = key[0];
                let user_session_id = key[1];
                match is_valid(user_id, user_session_id, server_state).await {
                    Ok(true) => Outcome::Success(UserSession(user_session_id)),
                    Ok(false) => {
                        info!("Could not find user session in user sessions.");
                        Outcome::Failure((
                            Status::BadRequest,
                            UserSessionError::Invalid,
                        ))
                    }
                    Err(e) => {
                        info!("Failed to verify user session: {e}");
                        Outcome::Failure((
                            Status::BadRequest,
                            UserSessionError::Invalid,
                        ))
                    }
                }
            }
        }
    }
}
