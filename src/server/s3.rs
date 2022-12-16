use super::{ApiResponse, ServerState};
use rocket::{http::Status, response::status::Custom, State};

/// Return an S3 bucket already existing
///
/// The bucket must have the ID `name`, be set in the region `region`
/// and be available at `endpoint`.
///
/// # Errors
///
/// The creation of credentials is not guaranteed and may error out,
/// in which case a `s3::error::S3Error` is returned to the caller of
/// the function. The variables `AWS_ACCESS_KEY_ID`,
/// `AWS_SECRET_ACCESS_KEY`, and `AWS_SESSION_TOKEN` must be set,
/// regardless if the S3 bucket is hosted by Amazon or not.
pub fn connect_to_bucket(
    name: &str,
    region: String,
    endpoint: String,
) -> Result<s3::Bucket, s3::error::S3Error> {
    s3::Bucket::new(
        name,
        s3::region::Region::Custom { region, endpoint },
        s3::creds::Credentials::default()?,
    )
}

/// Upload a file known as an array of `u8` to a S3 bucket
///
/// # Errors
///
/// The upload may fail for various resons. If this is the case, the
/// error is returned wrapped in a `Custom<String>` error.
pub async fn upload_file(
    state: &State<ServerState>,
    filename: String,
    file: &[u8],
) -> ApiResponse<()> {
    state
        .s3_bucket
        .put_object(format!("/{}", filename), file)
        .await
        .map(|_| info!("Uploaded file!"))
        .map_err(|e| {
            info!("Failed to upload file: {}", e);
            Custom(
                Status::InternalServerError,
                format!("Failed to upload file: {}", e),
            )
        })
}

/// Delete an object from the server's associated S3 bucket
///
/// Delete the object named `filename` located at the bucket's root.
///
/// # Errors
///
/// If the bucket fails to delete the object named `filename` for
/// whatever reason, it will error out. This error is wrapped in a
/// `Custom<String>` error and returned to the caller function.
pub async fn delete_file(
    state: &State<ServerState>,
    filename: String,
) -> ApiResponse<()> {
    state
        .s3_bucket
        .delete_object(format!("/{}", filename))
        .await
        .map(|_| {
            info!(
                "Removed remote object {} from S3 bucket {}",
                state.s3_bucket.name(),
                filename
            );
        })
        .map_err(|e| {
            Custom(
                Status::InternalServerError,
                format!(
                    "Failed to remove remote object {} from S3 bucket {}: {}",
                    state.s3_bucket.name(),
                    filename,
                    e
                ),
            )
        })
}
