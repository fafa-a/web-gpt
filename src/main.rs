use std::{io::{stdout }, sync::Arc};

use anyhow::Context;
use askama::Template;
use async_openai::{
    config::OpenAIConfig,
    types::{ChatCompletionRequestUserMessageArgs, CreateChatCompletionRequestArgs},
    Client,
};
use axum::{
     extract::State, http::StatusCode, response::{Html, IntoResponse, Response}, routing::{get, post}, Form, Json, Router
};
use serde::Deserialize;

use futures::StreamExt;
use std::io::Write;
use tokio::sync::Mutex;
use tower_http::services::ServeDir;
use tracing::info;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

struct AppState {
    queries: Mutex<Vec<String>>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "web_gpt=debug".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();
    info!("initializing router...");

    let api_key = ""; // This secret could be from a file, or environment variable.
    let config = OpenAIConfig::new().with_api_key(api_key);

    let client = Client::with_config(config);
    println!("client: {:?}", client);

    let request = CreateChatCompletionRequestArgs::default()
        .model("gpt-4-1106-preview")
        .max_tokens(512u16)
        .messages([ChatCompletionRequestUserMessageArgs::default()
            .content("how are you?")
            .build()?
            .into()])
        .build()?;

    let mut stream = client.chat().create_stream(request).await?;

    let mut lock = stdout().lock();
    
    while let Some(result) = stream.next().await {
        match result {
            Ok(response) => {
                response.choices.iter().for_each(|chat_choice| {
                    if let Some(ref content) = chat_choice.delta.content {
                        write!(lock, "{}", content).unwrap();
                    }
                });
            }
            Err(err) => {
                writeln!(lock, "error: {err}").unwrap();
            }
        }
        stdout().flush()?;
    }


    let app_state = Arc::new(AppState {
        queries: Mutex::new(vec![]),
    });
    let api_router = Router::new()
        .route("/submit", post(send_request))
        .route("/chat-response", post(handle_chat_response))
        .with_state(app_state);

    let assets_path = std::env::current_dir().unwrap();
    let router = Router::new()
        .nest("/api", api_router)
        .route("/", get(hello))
        .route("/response", get(my_div_handler))
        .nest_service(
            "/assets",
            ServeDir::new(format!("{}/assets", assets_path.to_str().unwrap())),
        );
    let port = 8000_u16;
    let addr = std::net::SocketAddr::from(([0, 0, 0, 0], port));

    info!("router initialized, now listening on port {}", port);

    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, router)
        .await
        .context("error while starting server")?;

    Ok(())
}

async fn hello() -> impl IntoResponse {
    let template = HelloTemplate {};
    HtmlTemplate(template)
}

#[derive(Template)]
#[template(path = "hello.html")]
struct HelloTemplate;



#[derive(Template)]
#[template(path = "query-list.html")]
struct QueryList {
    queries: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct QueryRequest {
    query: String,
}

async fn send_request(
    State(_state): State<Arc<AppState>>,
    Form(query): Form<QueryRequest>,
) -> impl IntoResponse {
    println!("query: {:?}", query);
    let mut lock = _state.queries.lock().await;

    lock.push(query.query);

    let template = QueryList {
        queries: lock.clone(),
    };

    HtmlTemplate(template)
}

#[derive(Debug)]
struct MyData {
    text: String,
}

// Define the Askama template
#[derive(Template)]
#[template(path = "response.html")]
struct MyDivTemplate {
    content: MyData,
}

// The Axum route handler
async fn my_div_handler(content: Json<String>) -> impl IntoResponse {
    let my_data = MyData {
        text: content.to_string()
    };
    Response::builder()
        .header("Content-Type", "application/json");

    let template = MyDivTemplate { content: my_data };

    match template.render() {
        Ok(body) => (StatusCode::OK, body),
        Err(_) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            "Internal server error".to_string(),
        ),
    }
}

#[derive(Debug,Deserialize)]
struct ChatResponse {
    content: String,
}

async fn handle_chat_response(chat_response: Json<ChatResponse>) -> impl IntoResponse {
    let content = chat_response.content.clone();
    let sse_response = format!("data: {}\n\n", content);
    axum::response::Html(sse_response)
}

/// A wrapper type that we'll use to encapsulate HTML parsed by askama into valid HTML for axum to serve.
struct HtmlTemplate<T>(T);

/// Allows us to convert Askama HTML templates into valid HTML for axum to serve in the response.
impl<T> IntoResponse for HtmlTemplate<T>
where
    T: Template,
{
    fn into_response(self) -> Response {
        // Attempt to render the template with askama
        match self.0.render() {
            // If we're able to successfully parse and aggregate the template, serve it
            Ok(html) => Html(html).into_response(),
            // If we're not, return an error or some bit of fallback HTML
            Err(err) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to render template. Error: {}", err),
            )
                .into_response(),
        }
    }
}
