use actix_web::body::MessageBody;
use awscreds;
use s3::error::S3Error;
use s3::Bucket;
use s3::Region;
use std::env;
use tokio;

fn create_bucket_from_env() -> Option<Bucket> {
    if let Ok(bucket_name) = env::var("DIRCAST_BUCKET_NAME") {
        if let Ok(credentials) = awscreds::Credentials::from_env() {
            if let Ok(bucket) = Bucket::new(
                bucket_name.as_str(),
                Region::EuCentral1,
                // Credentials are collected from environment, config, profile or instance metadata
                credentials,
            ) {
                return Some(bucket);
            } else {
                println!("Failed to create Bucket");
            }
        } else {
            println!("Failed to read AWS credentials from env.");
        }
    } else {
        println!("Env variable DIRCAST_BUCKET_NAME unset!");
    }

    None
}

#[tokio::main]
async fn main() -> std::io::Result<()> {
    let search = env::var("DIRCAST_SEARCH").unwrap_or("".to_string());
    if let Some(bucket) = create_bucket_from_env() {
        if let Ok(results) = bucket.list(search, None).await {
            for result in results {
                for item in result.contents {
                    println!("* {:?}", item);
                }
            }
        }
    }

    Ok(())
}
