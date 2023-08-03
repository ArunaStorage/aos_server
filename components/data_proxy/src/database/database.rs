use anyhow::Result;
use deadpool_postgres::{Config, ManagerConfig, Object, Pool, RecyclingMethod, Runtime};
use tokio_postgres::NoTls;
pub struct Database {
    connection_pool: Pool,
}

impl Database {
    pub fn new() -> Result<Self> {
        let database_host = dotenvy::var("PERSISTENCE_DB_HOST")?;
        let database_port = dotenvy::var("PERSISTENCE_DB_PORT")?.parse()?;
        let database_name = dotenvy::var("PERSISTENCE_DB_NAME")?;
        let database_user = dotenvy::var("PERSISTENCE_DB_USER")?;

        let mut cfg = Config::new();
        cfg.host = Some(database_host.to_string());
        cfg.port = Some(database_port);
        cfg.user = Some(database_user.to_string());
        cfg.dbname = Some(database_name.to_string());
        cfg.manager = Some(ManagerConfig {
            recycling_method: RecyclingMethod::Fast,
        });
        let pool = cfg.create_pool(Some(Runtime::Tokio1), NoTls)?;

        Ok(Database {
            connection_pool: pool,
        })
    }

    pub async fn initialize_db(&self) -> Result<()> {
        let client = self.connection_pool.get().await?;
        let initial = tokio::fs::read_to_string("./src/database/schema.sql").await?;
        client.batch_execute(&initial).await?;
        Ok(())
    }
}
