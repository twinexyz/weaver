## Twine Database

To compile twine sequencer, you need to create a postgresql database, and run the migrations.

### Prerequisities
We're using the macro `sqlx::query!()` for compile-time syntactic and semantic verification of sql. So, we need to install `sqlx` cli to run the migrations in order to be abe to use the macro,
```sh
cargo install sqlx-cli
```

## Running migrations

In the root of the project, start a postgres docker container.

```sh
docker compose up -d
```

Then, export the connection url of the postgresql database as
```sh
export DATABASE_URL=postgresql://twine_user:password@localhost:5432/twine_user
```
> Note: Or you can give your custom connection url

Now, run the migrations as
```sh
cd crates/database
sqlx migrate run
```

Then, you should be able to build the project.
