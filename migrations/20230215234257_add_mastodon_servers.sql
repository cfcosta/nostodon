create table mastodon_servers (
  id uuid primary key default uuid_generate_v4(),
  instance_url text not null,
  client_key text not null,
  client_secret text not null,
  token text not null,
  redirect_url text not null default 'urn:ietf:wg:oauth:2.0:oob'
);

create unique index if not exists mastodon_servers_client_key_unique_idx on mastodon_servers (client_key);
