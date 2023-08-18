use crate::database::crud::CrudDb;
use crate::database::dsls::user_dsl::{User, UserAttributes};
use crate::middlelayer::db_handler::DatabaseHandler;
use crate::middlelayer::user_request_types::{
    ActivateUser, DeactivateUser, RegisterUser, UpdateUserEmail, UpdateUserName,
};
use anyhow::{anyhow, Result};
use aruna_rust_api::api::notification::services::v2::EventVariant;
use diesel_ulid::DieselUlid;
use postgres_types::Json;
use tokio_postgres::GenericClient;

impl DatabaseHandler {
    pub async fn register_user(
        &self,
        request: RegisterUser,
        external_id: String,
    ) -> Result<(DieselUlid, User)> {
        let client = self.database.get_client().await?;
        let user_id = DieselUlid::generate();
        let new_attributes = UserAttributes {
            global_admin: false,
            service_account: false,
            tokens: Default::default(),
            trusted_endpoints: Default::default(),
            custom_attributes: vec![],
            permissions: Default::default(),
        };
        let mut user = User {
            id: user_id,
            display_name: request.get_display_name(),
            external_id: Some(external_id),
            email: request.get_email(),
            attributes: Json(new_attributes),
            active: false,
        };
        user.create(client.client()).await?;
        Ok((user_id, user))
    }
    pub async fn deactivate_user(&self, request: DeactivateUser) -> Result<(DieselUlid, User)> {
        let client = self.database.get_client().await?;
        let id = request.get_id()?;
        User::deactivate_user(&client, &id).await?;
        let user = User::get(id, &client)
            .await?
            .ok_or_else(|| anyhow!("User not found"))?;
        Ok((id, user))
    }
    pub async fn activate_user(&self, request: ActivateUser) -> Result<(DieselUlid, User)> {
        let client = self.database.get_client().await?;
        let id = request.get_id()?;
        User::activate_user(&client, &id).await?;
        let user = User::get(id, &client)
            .await?
            .ok_or_else(|| anyhow!("User not found"))?;
        Ok((id, user))
    }
    pub async fn update_display_name(
        &self,
        request: UpdateUserName,
        user_id: DieselUlid,
    ) -> Result<User> {
        let client = self.database.get_client().await?;
        let name = request.get_name();
        User::update_display_name(&client, &user_id, name).await?;
        let user = User::get(user_id, &client)
            .await?
            .ok_or_else(|| anyhow!("User not found"))?;
        Ok(user)
    }
    pub async fn update_email(
        &self,
        request: UpdateUserEmail,
        user_id: DieselUlid,
    ) -> Result<User> {
        let client = self.database.get_client().await?;
        let email = request.get_email();
        User::update_email(&client, &user_id, email).await?;
        let user = User::get(user_id, &client)
            .await?
            .ok_or_else(|| anyhow!("User not found"))?;
        Ok(user)
    }

    pub async fn add_endpoint_to_user(
        &self,
        user_id: DieselUlid,
        endpoint_id: DieselUlid,
    ) -> Result<()> {
        let client = self.database.get_client().await?;
        let user = User::add_trusted_endpoint(&client, &user_id, &endpoint_id).await?;

        self.natsio_handler
            .register_user_event(user, EventVariant::Updated)
            .await
    }
}
