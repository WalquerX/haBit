// src/main.rs
use axum::{extract::Json, http::StatusCode, response::IntoResponse, routing::post, Router};
use clap::{Parser, Subcommand};
use serde::{Deserialize, Serialize};
use tower_http::cors::CorsLayer;

mod nft;
use nft::*;

#[cfg(test)]
mod tests;

#[derive(Parser)]
#[command(name = "habit-tracker")]
#[command(about = "Habit Tracker NFT Manager", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Create a new habit tracker NFT
    Create {
        #[arg(short, long)]
        habit: String,
    },
    /// Update NFT (increment session counter)
    Update {
        #[arg(short, long)]
        utxo: String,
    },
    /// View NFT details
    View {
        #[arg(short, long)]
        utxo: String,
    },
}

// API Request/Response types
#[derive(Deserialize)]
struct CreateNftRequest {
    habit: String,
    address: String,
    funding_utxo: String,
    funding_value: u64,
}

// Request for broadcasting signed tx
#[derive(Deserialize)]
struct BroadcastNftRequest {
    signed_commit_hex: String,
    signed_spell_hex: String,
}

#[derive(Deserialize)]
struct UpdateNftRequest {
    nft_utxo: String,
    user_address: String,
    funding_utxo: String,
    funding_value: u64,
}

#[derive(Deserialize)]
struct ViewNftRequest {
    utxo: String,
}

// Generic response
#[derive(Serialize)]
struct ApiResponse<T> {
    success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    message: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    data: Option<T>,
}

impl<T: Serialize> IntoResponse for ApiResponse<T> {
    fn into_response(self) -> axum::response::Response {
        (StatusCode::OK, Json(self)).into_response()
    }
}

// API handlers
// async fn handle_create(
//     Json(req): Json<CreateNftRequest>,
// ) -> Result<ApiResponse, (StatusCode, String)> {
//     tokio::task::spawn_blocking(move || {
//         let btc = connect_bitcoin()?;
//         create_nft(&btc, req.habit)
//     })
//     .await
//     .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
//     .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

//     Ok(ApiResponse {
//         success: true,
//         message: "NFT created successfully".to_string(),
//         data: None,
//     })
// }

// Handler 1: Build unsigned transactions
async fn handle_create_unsigned(
    Json(req): Json<CreateNftRequest>,
) -> Result<ApiResponse<UnsignedNftResponse>, (StatusCode, String)> {
    let unsigned = tokio::task::spawn_blocking(move || {
        create_nft_unsigned(req.habit, req.address, req.funding_utxo, req.funding_value)
    })
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(ApiResponse {
        success: true,
        message: Some("Unsigned transactions created".to_string()),
        data: Some(unsigned),
    })
}

// Handler 2: Broadcast signed transactions
async fn handle_broadcast_nft(
    Json(req): Json<BroadcastNftRequest>,
) -> Result<ApiResponse<BroadcastNftResponse>, (StatusCode, String)> {
    let result = tokio::task::spawn_blocking(move || {
        let btc = connect_bitcoin()?;
        broadcast_nft(&btc, req.signed_commit_hex, req.signed_spell_hex)
    })
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(ApiResponse {
        success: true,
        message: Some("NFT broadcasted successfully".to_string()),
        data: Some(result),
    })
}

// Handler: Build unsigned update transactions
async fn handle_update_unsigned(
    Json(req): Json<UpdateNftRequest>,
) -> Result<ApiResponse<UnsignedUpdateResponse>, (StatusCode, String)> {
    let unsigned = tokio::task::spawn_blocking(move || {
        let btc = connect_bitcoin()?;
        update_nft_unsigned(
            &btc, // ← Pass it here
            req.nft_utxo,
            req.user_address,
            req.funding_utxo,
            req.funding_value,
        )
    })
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(ApiResponse {
        success: true,
        message: Some("Unsigned update transactions created".to_string()),
        data: Some(unsigned),
    })
}

// async fn handle_update(
//     Json(req): Json<UpdateNftRequest>,
// ) -> Result<ApiResponse, (StatusCode, String)> {
//     tokio::task::spawn_blocking(move || {
//         let btc = connect_bitcoin()?;
//         update_nft(&btc, req.utxo)
//     })
//     .await
//     .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
//     .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

//     Ok(ApiResponse {
//         success: true,
//         message: "NFT updated successfully".to_string(),
//         data: None,
//     })
// }

async fn handle_view(
    Json(req): Json<ViewNftRequest>,
) -> Result<ApiResponse<serde_json::Value>, (StatusCode, String)> {
    let utxo = req.utxo.clone();

    let (habit_name, sessions) = tokio::task::spawn_blocking(move || {
        let (txid, _vout) = utxo
            .split_once(':')
            .ok_or_else(|| anyhow::anyhow!("Invalid UTXO format, expected txid:vout"))?;

        let btc = connect_bitcoin()?;

        extract_nft_metadata(&btc, txid)
    })
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(ApiResponse {
        success: true,
        message: Some("NFT data retrieved".to_string()),
        data: Some(serde_json::json!({
            "utxo": req.utxo,
            "habit_name": habit_name,
            "sessions": sessions,
        })),
    })
}

// Server
async fn run_server() -> anyhow::Result<()> {
    let app = Router::new()
        .route("/api/nft/create/unsigned", post(handle_create_unsigned))
        .route("/api/nft/update/unsigned", post(handle_update_unsigned))
        .route("/api/nft/broadcast", post(handle_broadcast_nft))
        // .route("/api/nft/update", post(handle_update))
        .route("/api/nft/view", post(handle_view))
        .layer(CorsLayer::permissive());

    let listener = tokio::net::TcpListener::bind("127.0.0.1:3000").await?;
    println!("🚀 Habit Tracker API Server");
    println!("📍 Running on http://127.0.0.1:3000");
    println!("\n📝 API Endpoints:");
    println!("   POST /api/nft/create/unsigned - Build unsigned tx to create");
    println!("   POST /api/nft/update/unsigned - Build unsigned tx to update");
    println!("   POST /api/nft/broadcast - Broadcast signed tx");
    println!("   POST /api/nft/view - view an spell");
    axum::serve(listener, app).await?;
    Ok(())
}

// CLI
fn run_cli(command: Commands) -> anyhow::Result<()> {
    let btc = connect_bitcoin()?;

    match command {
        Commands::Create { habit } => create_nft(&btc, habit),
        Commands::Update { utxo } => update_nft(&btc, utxo),
        Commands::View { utxo } => view_nft(&btc, utxo), // ← Pass btc
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Some(cmd) => {
            // CLI mode
            run_cli(cmd)
        }
        None => {
            // Server mode
            run_server().await
        }
    }
}
