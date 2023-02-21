create extension if not exists "uuid-ossp";

create function fill_updated_at_on_update()
returns trigger as $$
begin
    new.updated_at = now();
    return new;
end;
$$ language 'plpgsql';

create table users (
    id uuid primary key default uuid_generate_v4(),
    instance_id uuid not null,
    nostr_public_key text not null,
    nostr_private_key text not null,
    mastodon_user text not null,
    created_at timestamptz not null default now(),
    updated_at timestamptz not null default now()
);

create unique index if not exists users_nostr_public_key_unique_idx on users (nostr_public_key);
create unique index if not exists users_mastodon_user_unique_idx on users (mastodon_user);
create trigger fill_users_updated_at_on_update before update on users for each row execute procedure fill_updated_at_on_update();

create table user_blacklists (
    id uuid primary key default uuid_generate_v4(),
    user_id uuid not null,
    created_at timestamptz not null default now(),
    updated_at timestamptz not null default now()
);

create trigger fill_user_blacklists_updated_at_on_update before update on user_blacklists for each row execute procedure fill_updated_at_on_update();

create table profiles (
    id uuid primary key default uuid_generate_v4(),
    instance_id uuid not null,
    user_id uuid not null,
    name text not null,
    display_name text not null,
    about text not null,
    picture text not null,
    nip05 text not null,
    banner text not null,
    created_at timestamptz not null default now(),
    updated_at timestamptz not null default now()
);

create unique index if not exists profiles_user_id_unique_idx on profiles (user_id);
create trigger fill_profiles_updated_at_on_update before update on profiles for each row execute procedure fill_updated_at_on_update();

create type mastodon_post_status as enum('posted', 'deleted');

create table mastodon_posts (
    id uuid primary key default uuid_generate_v4(),
    instance_id uuid not null,
    user_id uuid not null,
    mastodon_id text not null,
    nostr_id text not null,
    in_reply_to text,
    status mastodon_post_status not null,
    created_at timestamptz not null default now(),
    updated_at timestamptz not null default now()
);


create unique index if not exists mastodon_posts_mastodon_id_unique_idx on mastodon_posts (mastodon_id);
create unique index if not exists mastodon_posts_nostr_id_unique_idx on mastodon_posts (nostr_id);
create index if not exists mastodon_posts_in_reply_to_idx on mastodon_posts (in_reply_to);
create trigger fill_mastodon_posts_updated_at_on_update before update on mastodon_posts for each row execute procedure fill_updated_at_on_update();

create table mastodon_instances (
    id uuid primary key default uuid_generate_v4(),
    url text not null,
    blacklisted boolean not null default false,
    created_at timestamptz not null default now(),
    updated_at timestamptz not null default now()
);

create unique index if not exists mastodon_instances_url_unique_idx on mastodon_instances (url);
create index mastodon_isntances_blacklisted_idx on mastodon_instances (blacklisted);
create trigger fill_mastodon_instances_updated_at_on_update before update on mastodon_instances for each row execute procedure fill_updated_at_on_update();
