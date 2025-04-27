create table device_config_store (
    dev_eui bytea primary key references device on delete cascade,
    created_at timestamp with time zone not null,
    updated_at timestamp with time zone not null,
    chmask_config bytea
);
