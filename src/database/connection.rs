use anyhow::Result;
use deadpool_postgres::{Config, ManagerConfig, Object, Pool, RecyclingMethod, Runtime};
use tokio_postgres::NoTls;

pub struct Database {
    connection_pool: Pool,
}

impl Database {
    pub fn new(
        database_host: String,
        database_port: u16,
        database_name: String,
        database_user: String,
    ) -> Result<Self> {
        let mut cfg = Config::new();
        cfg.host = Some(database_host);
        cfg.port = Some(database_port);
        cfg.user = Some(database_user);
        cfg.dbname = Some(database_name);
        cfg.manager = Some(ManagerConfig {
            recycling_method: RecyclingMethod::Fast,
        });
        let pool = cfg.create_pool(Some(Runtime::Tokio1), NoTls)?;

        Ok(Database {
            connection_pool: pool,
        })
    }

    /*
    pub fn new (conn_str: &str) -> Result<Self> {
        let conf = tokio_postgres::Config
    }
    */

    pub async fn initialize_db(&self) -> Result<()> {
        let client = self.connection_pool.get().await?;
        dbg!(&client);

        let initial = tokio::fs::read_to_string("./src/database/schema.sql").await?;
        client.batch_execute(&initial).await?;
        Ok(())
    }

    pub async fn get_client(&self) -> Result<Object> {
        Ok(self.connection_pool.get().await?)
    }
}
