create table nostr_relays (
  id uuid primary key default uuid_generate_v4(),
  url text not null,
  created_at timestamptz not null default now(),
  updated_at timestamptz not null default now()
);

create unique index if not exists nostr_relays_client_key_unique_idx on nostr_relays (url);
create trigger fill_nostr_relays_updated_at_on_update before update on nostr_relays for each row execute procedure fill_updated_at_on_update();
