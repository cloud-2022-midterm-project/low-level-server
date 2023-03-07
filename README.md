# Low-level Server

This is a low-level, destined to fail, overengineered server for cloud computing midterm.

## How to run

### INSTALL RUST and cargo

<https://www.rust-lang.org/tools/install>

### .env

Create a `.env` file and fill it (see .env.example).

### Create local database

With `DATABASE_URL=postgres://postgres:mysecretpassword@localhost:5432/postgres` in `.env`

```bash
docker run --name postgres -e POSTGRES_PASSWORD=mysecretpassword -p 5432:5432 -d postgres
```

### Migration

Do this after the database is created to create tables.

```bash
cargo install sqlx-cli
sqlx migrate run
```

### Start the server

Debug mode:

```bash
cargo r
```

Release mode:

```bash
cargo r -r
```

![image](https://raw.githubusercontent.com/rochacbruno/rust_memes/master/img/python_for_kids.jpg)
![image](https://programmerhumor.io/wp-content/uploads/2022/01/programmerhumor-io-programming-memes-588f11d944783ab.png)
![image](https://raw.githubusercontent.com/rochacbruno/rust_memes/master/img/dontpanic.jpg)
