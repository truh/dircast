use actix_web::{
    cookie::Cookie, get, post, web, App, HttpRequest, HttpResponse, HttpServer, Responder,
};
use rss::{ChannelBuilder, Item};
use s3::Bucket;
use s3::Region;
use serde::Deserialize;
use serde::Serialize;
use std::cmp::Ordering;
use std::env;
use std::fs;
use tera::Tera;

#[derive(Deserialize)]
struct AuthStruct {
    user: String,
    pass: String,
}

#[derive(Deserialize)]
struct SearchStruct {
    author: String,
    search: String,
    title: String,
}

#[derive(Eq, Serialize, PartialEq)]
struct FileObject {
    name: String,
    url: String,
    e_tag: Option<String>,
    key: String,
    length: u64,
    mime_type: Option<String>,
}

impl Ord for FileObject {
    fn cmp(&self, other: &Self) -> Ordering {
        self.name.cmp(&other.name)
    }
}

impl PartialOrd for FileObject {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

#[derive(Deserialize, Serialize)]
struct SlugData {
    author: String,
    search: String,
    title: String,
    user: String,
    pass: String,
}

struct AppConfig {
    public_url: String,
}

fn create_bucket_from_env() -> Option<Bucket> {
    if let Ok(bucket_name) = env::var("DIRCAST_BUCKET_NAME") {
        if let Ok(credentials) = awscreds::Credentials::from_env() {
            if let Ok(bucket) = Bucket::new(bucket_name.as_str(), Region::EuCentral1, credentials) {
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

async fn bucket_search(bucket: &Bucket, query: String) -> Vec<FileObject> {
    let mut v: Vec<FileObject> = Vec::new();
    if let Ok(results) = bucket.list(query, None).await {
        for result in results {
            for item in result.contents {
                if let Ok(url) = bucket.presign_get(item.key.as_str(), 86400, None) {
                    v.push(FileObject {
                        name: item.key.clone(),
                        url,
                        e_tag: item.e_tag,
                        key: item.key,
                        length: item.size,
                        mime_type: Some(String::from("audio/mp3")),
                    });
                }
            }
        }
    }
    v.sort();
    v
}

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
async fn web_search_results(
    req: HttpRequest,
    form: web::Form<SearchStruct>,
    templates: web::Data<tera::Tera>,
    app_config: web::Data<AppConfig>,
) -> impl Responder {
    let search_struct = form.into_inner();

    if let Some(auth) = check_auth_cookie(req) {
        if let Some(bucket) = create_bucket_from_env() {
            let search_results = bucket_search(&bucket, search_struct.search.clone()).await;
            let mut context = tera::Context::new();

            let slug_data = SlugData {
                author: search_struct.author.clone(),
                search: search_struct.search.clone(),
                title: search_struct.title.clone(),
                user: auth.user.clone(),
                pass: auth.pass.clone(),
            };
            if let Ok(slug_json) = serde_json::to_string(&slug_data) {
                context.insert("search_results", &search_results);

                let slug_b64 = base64_url::encode(&slug_json);
                let feed_url = format!("{}/gen_feed/{}/feed.rss", app_config.public_url, slug_b64);
                context.insert("feed_url", &feed_url);

                let author_query = search_struct.author;
                context.insert("author_query", &author_query);

                let search_query = search_struct.search;
                context.insert("search_query", &search_query);

                let title_query = search_struct.title;
                context.insert("title_query", &title_query);

                return match templates.render("search.html", &context) {
                    Ok(s) => HttpResponse::Ok().content_type("text/html").body(s),
                    Err(e) => {
                        println!("{:?}", e);
                        HttpResponse::InternalServerError()
                            .content_type("text/html")
                            .body("<p>Something went wrong!</p>")
                    }
                };
            }
        }
        return HttpResponse::InternalServerError()
            .content_type("text/html")
            .body("<p>Something went wrong!</p>");
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

#[get("/gen_feed/{slug}/feed.rss")]
async fn gen_feed(params: web::Path<String>, app_config: web::Data<AppConfig>) -> impl Responder {
    let slug_b64 = params.into_inner();
    if let Ok(slug_json_b) = base64_url::decode(&slug_b64) {
        if let Ok(slug_json) = std::str::from_utf8(&slug_json_b) {
            let slug_data_result: serde_json::Result<SlugData> = serde_json::from_str(slug_json);
            if let Ok(slug_data) = slug_data_result {
                let auth_struct = AuthStruct {
                    user: slug_data.user,
                    pass: slug_data.pass,
                };
                let title = slug_data.title.clone();
                let author = slug_data.author.clone();
                if check_auth(&auth_struct) {
                    if let Some(bucket) = create_bucket_from_env() {
                        let search_results = bucket_search(&bucket, slug_data.search.clone()).await;
                        let mut channel: rss::Channel = ChannelBuilder::default()
                            .title(slug_data.title)
                            // .link("")
                            // .description("")
                            .build();
                        let mut items: Vec<Item> = Vec::new();
                        for (i, file_object) in search_results.iter().enumerate() {
                            let mut enclosure = rss::Enclosure::default();
                            enclosure.set_url(file_object.url.clone());
                            enclosure.set_length(format!("{}", file_object.length));
                            if let Some(mime_type) = &file_object.mime_type {
                                enclosure.set_mime_type(mime_type.clone());
                            }
                            let mut guid = rss::Guid::default();
                            guid.set_value(file_object.key.clone());
                            items.push(Item {
                                title: Some(format!("{} {}", &title, i + 1)),
                                link: Some(app_config.public_url.clone()),
                                description: Some(String::from("")),
                                author: Some(author.clone()),
                                categories: vec![],
                                enclosure: Some(enclosure),
                                guid: Some(guid),
                                comments: None,
                                pub_date: None,
                                source: None,
                                content: None,
                                extensions: Default::default(),
                                itunes_ext: None,
                                dublin_core_ext: None,
                            });
                        }
                        channel.items = items;
                        return HttpResponse::Ok()
                            .content_type("application/rss+xml")
                            .body(channel.to_string());
                    }
                } else {
                    return HttpResponse::Unauthorized()
                        .content_type("text/html")
                        .body("<p>Unauthorized</p>");
                }
            }
        }
    }

    HttpResponse::InternalServerError()
        .content_type("text/html")
        .body("<p>Something went wrong!</p>")
}

#[tokio::main]
async fn main() -> std::io::Result<()> {
    let host = env::var("DIRCAST_HOST").unwrap_or_else(|_| String::from("127.0.0.1"));
    let port = env::var("DIRCAST_PORT")
        .map(|e| e.parse::<u16>())
        .unwrap_or(Ok(8080))
        .unwrap_or(8080);
    let public_url =
        env::var("DIRCAST_PUBLIC_URL").unwrap_or_else(|_| format!("http://{}:{}", host, port));

    let templates = Tera::new("templates/**/*").unwrap();

    HttpServer::new(move || {
        let app_config = AppConfig {
            public_url: String::from(public_url.as_str()),
        };
        App::new()
            .app_data(web::Data::new(templates.clone()))
            .app_data(web::Data::new(app_config))
            .service(web_search)
            .service(web_search_results)
            .service(login_get)
            .service(login_post)
            .service(gen_feed)
            .service(styles_css)
    })
    .bind((host.as_str(), port))?
    .run()
    .await
}
