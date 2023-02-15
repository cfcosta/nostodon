create table mastodon_tokens (
  id uuid primary key default uuid_generate_v4(),
  instance_id uuid not null,
  client_key text not null,
  client_secret text not null,
  redirect_url text default 'urn:ietf:wg:oauth:2.0:oob'
);

create unique index if not exists mastodon_tokens_client_key_unique_idx on mastodon_tokens (client_key);
