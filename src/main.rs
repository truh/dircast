use actix_web::{
    cookie::Cookie, get, post, web, App, HttpRequest, HttpResponse, HttpServer, Responder,
};
// use awscreds;
// use rss::{ChannelBuilder, Item};
// use s3::Bucket;
// use s3::Region;
use serde::Deserialize;
use std::env;
use std::fs;
use tera::Tera;

#[derive(Deserialize)]
struct AuthStruct {
    user: String,
    pass: String,
}

// fn create_bucket_from_env() -> Option<Bucket> {
//     if let Ok(bucket_name) = env::var("DIRCAST_BUCKET_NAME") {
//         if let Ok(credentials) = awscreds::Credentials::from_env() {
//             if let Ok(bucket) = Bucket::new(
//                 bucket_name.as_str(),
//                 Region::EuCentral1,
//                 // Credentials are collected from environment, config, profile or instance metadata
//                 credentials,
//             ) {
//                 return Some(bucket);
//             } else {
//                 println!("Failed to create Bucket");
//             }
//         } else {
//             println!("Failed to read AWS credentials from env.");
//         }
//     } else {
//         println!("Env variable DIRCAST_BUCKET_NAME unset!");
//     }
//
//     None
// }

fn check_auth(auth_struct: &AuthStruct) -> bool {
    let dot_htpasswd =
        fs::read_to_string(".htpasswd").expect("Should have been able to read the file");
    let htpasswd = htpasswd_verify::load(&dot_htpasswd);
    htpasswd.check(&auth_struct.user, &auth_struct.pass)
}

fn check_auth_cookie(req: HttpRequest) -> Option<AuthStruct> {
    if let Some(cookie) = req.cookie("auth") {
        let value = cookie.value();
        let split = value.split(':').collect::<Vec<&str>>();
        if split.len() == 2 {
            let user = split[0];
            let pass = split[1];
            let auth_struct = AuthStruct {
                user: user.to_string(),
                pass: pass.to_string(),
            };
            if check_auth(&auth_struct) {
                return Some(auth_struct);
            }
        }
    }
    None
}

#[get("/")]
async fn web_search(req: HttpRequest, templates: web::Data<tera::Tera>) -> impl Responder {
    let context = tera::Context::new();

    if check_auth_cookie(req).is_some() {
        match templates.render("search.html", &context) {
            Ok(s) => HttpResponse::Ok().content_type("text/html").body(s),
            Err(e) => {
                println!("{:?}", e);
                HttpResponse::InternalServerError()
                    .content_type("text/html")
                    .body("<p>Something went wrong!</p>")
            }
        }
    } else {
        HttpResponse::TemporaryRedirect()
            .insert_header(("Location", "/login"))
            .finish()
    }
}

#[post("/")]
async fn web_search_results(req: HttpRequest, templates: web::Data<tera::Tera>) -> impl Responder {
    let context = tera::Context::new();

    if check_auth_cookie(req).is_some() {
        match templates.render("search.html", &context) {
            Ok(s) => HttpResponse::Ok().content_type("text/html").body(s),
            Err(e) => {
                println!("{:?}", e);
                HttpResponse::InternalServerError()
                    .content_type("text/html")
                    .body("<p>Something went wrong!</p>")
            }
        }
    } else {
        HttpResponse::TemporaryRedirect()
            .insert_header(("Location", "/login"))
            .finish()
    }
}

#[get("/login")]
async fn login_get(req: HttpRequest, templates: web::Data<tera::Tera>) -> impl Responder {
    let context = tera::Context::new();

    if check_auth_cookie(req).is_some() {
        HttpResponse::TemporaryRedirect()
            .insert_header(("Location", "/"))
            .finish()
    } else {
        match templates.render("login.html", &context) {
            Ok(s) => HttpResponse::Ok().content_type("text/html").body(s),
            Err(e) => {
                println!("{:?}", e);
                HttpResponse::InternalServerError()
                    .content_type("text/html")
                    .body("<p>Something went wrong!</p>")
            }
        }
    }
}

#[post("/login")]
async fn login_post(
    req: HttpRequest,
    form: web::Form<AuthStruct>,
    templates: web::Data<tera::Tera>,
) -> impl Responder {
    let auth_f = form.into_inner();
    let context = tera::Context::new();

    if check_auth_cookie(req).is_some() {
        return HttpResponse::TemporaryRedirect()
            .insert_header(("Location", "/"))
            .finish();
    }

    if check_auth(&auth_f) {
        let mut auth_value: String = "".to_owned();
        auth_value.push_str(&auth_f.user);
        auth_value.push(':');
        auth_value.push_str(&auth_f.pass);
        return HttpResponse::TemporaryRedirect()
            .cookie(Cookie::build("auth", auth_value.as_str()).finish())
            .insert_header(("Location", "/"))
            .finish();
    }

    match templates.render("login.html", &context) {
        Ok(s) => HttpResponse::Unauthorized()
            .content_type("text/html")
            .body(s),
        Err(e) => {
            println!("{:?}", e);
            HttpResponse::InternalServerError()
                .content_type("text/html")
                .body("<p>Something went wrong!</p>")
        }
    }
}

#[get("/styles.css")]
async fn styles_css(templates: web::Data<tera::Tera>) -> impl Responder {
    let context = tera::Context::new();
    match templates.render("styles.css", &context) {
        Ok(s) => HttpResponse::Ok().content_type("text/css").body(s),
        Err(e) => {
            println!("{:?}", e);
            HttpResponse::InternalServerError()
                .content_type("text/html")
                .body("<p>Something went wrong!</p>")
        }
    }
}

#[tokio::main]
async fn main() -> std::io::Result<()> {
    let host = env::var("DIRCAST_HOST").unwrap_or_else(|_| String::from("127.0.0.1"));
    let port = env::var("DIRCAST_PORT")
        .map(|e| e.parse::<u16>())
        .unwrap_or(Ok(8080))
        .unwrap_or(8080);
    // let search = env::var("DIRCAST_SEARCH").unwrap_or("".to_string());
    // if let Some(bucket) = create_bucket_from_env() {
    //     if let Ok(results) = bucket.list(search, None).await {
    //         for result in results {
    //             for item in result.contents {
    //                 println!("* {:?}", item);
    //             }
    //         }
    //     }
    // }

    let templates = Tera::new("templates/**/*").unwrap();

    HttpServer::new(move || {
        App::new()
            .app_data(web::Data::new(templates.clone()))
            .service(web_search)
            .service(login_get)
            .service(login_post)
            .service(styles_css)
    })
    .bind((host.as_str(), port))?
    .run()
    .await
}
