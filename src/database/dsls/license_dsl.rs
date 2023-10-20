use crate::database::crud::{CrudDb, PrimaryKey};
use anyhow::anyhow;
use anyhow::Result;
use async_trait::async_trait;
use postgres_from_row::FromRow;
use postgres_types::{FromSql, Json, ToSql};
use serde::{Deserialize, Serialize};
use tokio_postgres::Client;

#[derive(FromRow, Debug, Clone, PartialEq)]
pub struct License {
    pub tag: String,
    pub name: String,
    pub description: String,
    pub url: String,
}

pub const ALL_RIGHTS_RESERVED: &str = "all_rights_reserved";

#[async_trait]
impl CrudDb for License {
    async fn create(&mut self, client: &Client) -> Result<()> {
        let query = "INSERT INTO licenses (tag, name, description, url) 
        VALUES ( $1, $2, $3, $4) 
        RETURNING *;";

        let prepared = client.prepare(query).await?;

        let row = client
            .query_one(
                &prepared,
                &[&self.tag, &self.name, &self.description, &self.url],
            )
            .await?;

        *self = License::from_row(&row);
        Ok(())
    }
    async fn get(tag: impl PrimaryKey, client: &Client) -> Result<Option<Self>> {
        let query = "SELECT * FROM licenses WHERE tag = $1";
        let prepared = client.prepare(query).await?;
        Ok(client
            .query_opt(&prepared, &[&tag])
            .await?
            .map(|e| License::from_row(&e)))
    }
    async fn all(client: &Client) -> Result<Vec<Self>> {
        let query = "SELECT * FROM licenses";
        let prepared = client.prepare(query).await?;
        let rows = client.query(&prepared, &[]).await?;
        Ok(rows.iter().map(License::from_row).collect::<Vec<_>>())
    }
    async fn delete(&self, client: &Client) -> Result<()> {
        let query = "DELETE FROM licenses WHERE id = $1;";
        let prepared = client.prepare(query).await?;
        client.execute(&prepared, &[&self.tag]).await?;
        Ok(())
    }
}
