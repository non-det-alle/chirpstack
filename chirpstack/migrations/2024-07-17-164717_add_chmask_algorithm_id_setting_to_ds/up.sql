alter table device_profile
  add column chmask_algorithm_id varchar(100) not null;

alter table device_profile_template
  add column chmask_algorithm_id varchar(100) not null;
