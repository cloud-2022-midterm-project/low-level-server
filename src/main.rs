use ahash::AHashSet;
use dotenv::dotenv;
use futures_util::stream::StreamExt;
use server_low_level::{app_state::AppState, handle_connection, mutation_manager::MutationManager};
use sqlx::postgres::PgPoolOptions;
use std::{net::SocketAddr, sync::Arc};
use tokio::{
    net::TcpListener,
    signal,
    sync::{mpsc, Mutex},
};

#[tokio::main]
async fn main() {
    dotenv().ok();

    println!("Connecting to database...");
    let db_pool = Arc::new(
        match PgPoolOptions::new()
            .min_connections(90)
            .max_connections(100)
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

    let pagination_page_size: usize = std::env::var("PAGINATION_PAGE_SIZE")
        .expect("PAGINATION_PAGE_SIZE is not set")
        .parse()
        .expect("PAGINATION_PAGE_SIZE must be a number");

    // setting up the tcp listener

    // the state of the tcp listener server
    let state = Arc::new(AppState {
        pool: db_pool,
        mutations: Mutex::new(MutationManager::new(pagination_page_size)),
        pagination_page_size,
        db_pagination_offset: Mutex::new(0),
        triggered_pagination: Mutex::new(false),
        image_base_path: {
            let path = std::env::var("IMAGES_BASE_PATH").expect("IMAGES_BASE_PATH must be set");
            let path = std::path::Path::new(&path);
            // check if this path directory exists
            if !std::path::Path::new(&path).exists() {
                panic!("IMAGES_BASE_PATH directory does not exist, the given path is {path:#?}.");
            }
            path.to_path_buf()
        },
        all_uuids: {
            let mut uuids = AHashSet::with_capacity(50_000usize.next_power_of_two());
            let mut stream = sqlx::query!("SELECT uuid FROM messages")
                .map(|row| row.uuid)
                .fetch(db_pool_cloned.as_ref());
            while let Some(uuid) = stream.next().await {
                let uuid = uuid.expect("Failed to fetch uuid from database");
                uuids.insert(uuid);
            }
            println!("Fetched all {} uuids from database.", uuids.len());
            Mutex::new(uuids)
        },
        pagination_page_number: Mutex::new(0),
        pages_count: Mutex::new(0),
    });

    // the address to bind to
    let addr = SocketAddr::from((
        [0, 0, 0, 0],
        std::env::var("PORT")
            .unwrap_or("3000".to_string())
            .parse()
            .expect("PORT must be a number"),
    ));

    // the tcp listener
    let listener = match TcpListener::bind(addr).await {
        Ok(listener) => {
            println!("Listening on {}", listener.local_addr().unwrap());
            listener
        }
        Err(e) => {
            panic!("Failed to bind to {}: {}", addr, e);
        }
    };

    // the main task that listens for incoming HTTP requests
    let listener_task = async move {
        loop {
            // select between the listener accepting a new connection and the shutdown signal
            tokio::select! {
                // a new connection has been accepted
                result = listener.accept() => {
                    match result {
                        Ok((stream, _)) => {
                            tokio::spawn(handle_connection(stream, Arc::clone(&state)));
                        }
                        Err(e) => {
                            eprintln!("Failed to accept connection: {}", e);
                        },
                    }
                }
                // the shutdown signal has been received
                _ = shutdown_recv.recv() => {
                    break;
                }
            }
        }
    };

    // spawn the tcp listener thread
    let tcp_listener_thread = tokio::spawn(listener_task);

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
    shutdown_send.send(()).ok();

    // wait for the tcp listener to finish
    tcp_listener_thread
        .await
        .expect("Failed to join server task");
}
