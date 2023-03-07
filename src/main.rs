use dotenv::dotenv;
use server_low_level::{
    app_state::{mutation_manager::MutationManager, AppState},
    handle_connection,
    image_store::ImageStore,
};
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
        image_store: ImageStore::new(),
        mutations: Mutex::new(MutationManager::new(
            pagination_page_size,
            ImageStore::new(),
        )),
        pagination_page_size,
        db_pagination_offset: Mutex::new(0),
        triggered_pagination: Mutex::new(false),
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
            eprintln!("Failed to bind to {}: {}", addr, e);
            std::process::exit(1);
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
