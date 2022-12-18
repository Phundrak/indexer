use rocket::http::Status;
use rocket::request::{FromRequest, Outcome, Request};
use rocket::serde::Deserialize;

use super::ServerState;

#[derive(Clone, Deserialize)]
#[serde(crate = "rocket::serde")]
struct APISession {
    #[serde(rename(deserialize = "$id"))]
    id: String,
}

#[derive(Clone, Deserialize)]
#[serde(crate = "rocket::serde")]
struct APIUserSession {
    session: APISession,
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
        ) -> bool {
            let client = reqwest::Client::new();
            let url = format!(
                "{}/users/{}/sessions",
                state.appwrite_endpoint, user_id
            );
            let response = client
                .get(url.clone())
                .header("X-Appwrite-Key", state.appwrite_key.clone())
                .header("X-Appwrite-Project", state.appwrite_project.clone())
                .header("Content-Type", "application/json")
                .send()
                .await;
            if response.is_err() {
                info!(
                    "Could not perform GET request to {}: {}",
                    url,
                    response.err().unwrap()
                );
                return false;
            }
            match response.unwrap().json::<Vec<APIUserSession>>().await {
                Err(e) => {
                    info!(
                        "Could not retrieve JSON response from {}: {}",
                        url, e
                    );
                    false
                }
                Ok(val) => val.iter().any(|s| s.session.id == user_session_id),
            }
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
                if is_valid(user_id, user_session_id, server_state).await {
                    Outcome::Success(UserSession(user_session_id))
                } else {
                    Outcome::Failure((
                        Status::BadRequest,
                        UserSessionError::Invalid,
                    ))
                }
            }
        }
    }
}
