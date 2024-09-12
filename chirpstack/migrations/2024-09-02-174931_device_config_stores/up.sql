create table device_config_store (
    dev_eui bytea references device on delete cascade primary key,
    created_at timestamp with time zone not null,
    updated_at timestamp with time zone not null,
    chmask_config bytea
);
