use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(Files::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(Files::Name)
                            .string()
                            .not_null()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(Files::Crc32).integer().not_null())
                    .col(ColumnDef::new(Files::Contents).string().not_null())
                    .col(
                        ColumnDef::new(Files::History)
                            .array(ColumnType::Text)
                            .not_null(),
                    )
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(Files::Table).to_owned())
            .await
    }
}

/// Learn more at https://docs.rs/sea-query#iden
#[derive(Iden)]
enum Files {
    Table,
    Name,
    Contents,
    Crc32,
    History,
}
