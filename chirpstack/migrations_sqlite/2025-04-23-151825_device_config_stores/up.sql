create table device_config_store (
    dev_eui blob not null primary key references device on delete cascade,
    created_at datetime not null,
    updated_at datetime not null,
    chmask_config blob not null,
    dr smallint,
    tx_power_index smallint,
    nb_trans smallint,
    max_duty_cycle smallint
);
