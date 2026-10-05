//! The copy method of the service (spec 036, FR-015, research R7): for the user alone (Z11), in one
//! write, so a copy of hundreds of entries is all or nothing (FR-022).

use super::{require_user, PasswordsService};
use crate::error::Result;
use crate::passwords::access::Caller;
use crate::passwords::copy::{self, CopyOptions, CopyReport};
use crate::passwords::model::Target;

impl PasswordsService {
    pub async fn copy(
        &self,
        caller: &Caller,
        targets: Vec<Target>,
        into_group_id: Option<String>,
        options: CopyOptions,
    ) -> Result<CopyReport> {
        require_user(caller)?;
        self.db()
            .write(move |tx| {
                copy::copy(tx, &targets, into_group_id.as_deref(), &options).map_err(Into::into)
            })
            .await
    }
}
