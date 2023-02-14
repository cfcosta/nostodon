create extension if not exists "uuid-ossp";

create table users (
    id uuid primary key default uuid_generate_v4(),
    instance_id uuid not null,
    nostr_public_key text not null,
    nostr_private_key text not null,
    mastodon_user text not null
);

create unique index if not exists users_nostr_public_key_unique_idx on users (nostr_public_key);
create unique index if not exists users_mastodon_user_unique_idx on users (mastodon_user);

create table user_blacklists (
    id uuid primary key default uuid_generate_v4(),
    user_id uuid not null
);

create table profiles (
    id uuid primary key default uuid_generate_v4(),
    instance_id uuid not null,
    user_id uuid not null,
    name text not null,
    display_name text not null,
    about text not null,
    picture text not null,
    nip05 text not null
);

create unique index if not exists profiles_user_id_unique_idx on profiles (user_id);

create type mastodon_post_status as enum('posted', 'deleted');

create table mastodon_posts (
    id uuid primary key default uuid_generate_v4(),
    instance_id uuid not null,
    user_id uuid not null,
    mastodon_id text not null,
    nostr_id text not null,
    status mastodon_post_status not null
);


create unique index if not exists mastodon_posts_mastodon_id_unique_idx on mastodon_posts (mastodon_id);
create unique index if not exists mastodon_posts_nostr_id_unique_idx on mastodon_posts (nostr_id);

create table mastodon_instances (
    id uuid primary key default uuid_generate_v4(),
    url text not null,
    blacklisted boolean not null
);

create unique index if not exists mastodon_instances_url_unique_idx on mastodon_instances (url);
create index mastodon_isntances_blacklisted_idx on mastodon_instances (blacklisted);
