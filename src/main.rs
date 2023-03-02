use ahash::AHashMap;
use dotenv::dotenv;
use server_low_level::{app_state::AppState, handle_request, image_store::ImageStore};
use sqlx::postgres::PgPoolOptions;
use std::{net::SocketAddr, sync::Arc};
use tokio::{net::TcpListener, signal, sync::mpsc};

#[tokio::main]
async fn main() {
    dotenv().ok();

    println!("Connecting to database...");
    let db_pool = Arc::new(
        match PgPoolOptions::new()
            .max_connections(1)
            .connect(&std::env::var("DATABASE_URL").expect("DATABASE_URL is not set"))
            .await
        {
            Ok(pool) => {
                println!("Connected to database.");
                pool
            }
            Err(e) => {
                panic!("Failed to connect to database: {}", e);
            }
        },
    );
    let db_pool_cloned = Arc::clone(&db_pool);

    // channel to send shutdown signal to the tcp listener
    let (shutdown_send, mut shutdown_recv) = mpsc::unbounded_channel();

    // the main task that handles the tcp listener
    let listener_task = async move {
        let addr = SocketAddr::from((
            [0, 0, 0, 0],
            std::env::var("PORT")
                .unwrap_or("3000".to_string())
                .parse()
                .expect("PORT must be a number"),
        ));

        let listener = TcpListener::bind(addr)
            .await
            .expect(format!("Failed to bind to {}", addr).as_str());

        let mut state = AppState {
            pool: Arc::clone(&db_pool),
            image_store: ImageStore::new({
                let path = std::env::var("IMAGES_BASE_PATH").expect("IMAGES_BASE_PATH must be set");
                // check if this path directory exists
                if !std::path::Path::new(&path).exists() {
                    panic!("IMAGES_BASE_PATH directory does not exist, the given path is {path}.");
                }
                path
            }),
            updates_post: AHashMap::with_capacity(1000),
            updates_put: AHashMap::with_capacity(1000),
            updates_delete: Vec::with_capacity(1000),
        };

        println!("Listening on {}", addr);
        loop {
            tokio::select! {
                result = listener.accept() => {
                    match result {
                        Ok((mut stream, _)) => {
                            handle_request(&mut stream, &mut state).await;
                        }
                        Err(e) => {
                            eprintln!("Failed to accept connection: {}", e);
                        },
                    }
                }
                _ = shutdown_recv.recv() => {
                    break;
                }
            }
        }
    };

    // spawn the tcp listener thread
    let handle = tokio::spawn(listener_task);

    // leaving main thread to handle shutdown signal

    let ctrl_c = async {
        signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        signal::unix::signal(signal::unix::SignalKind::terminate())
            .expect("failed to install signal handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    // wait for any of the termination signals
    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }

    println!("Shutting down...");

    // close the database connection
    db_pool_cloned.close().await;
    println!("Database connection closed.");

    // send shutdown signal to the tcp listener
    shutdown_send
        .send(())
        .expect("Failed to send shutdown signal");

    // wait for the tcp listener to finish
    handle.await.expect("Failed to join server task");
}
