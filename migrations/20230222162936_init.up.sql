-- Add migration script here
CREATE TABLE messages (
    uuid char(36) unique not null,
    author varchar(64) not null,
    message varchar(1024) not null,
    likes int not null,
    has_image boolean not null
);